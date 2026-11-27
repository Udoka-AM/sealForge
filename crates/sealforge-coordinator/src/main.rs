use anyhow::Context;
use clap::Parser;
use sealforge_api::types::Network;
use sealforge_coordinator::{
    auth, closer, config::Config, ratelimit, refunds, retention, routes, scanner, AppState,
    Settings,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .json()
        .init();

    let config = Config::parse();

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;
    sqlx::migrate!("../../migrations")
        .run(&db)
        .await
        .context("running migrations")?;

    let state =
        AppState::new(
            db,
            Settings {
                invite_codes: config.invite_codes(),
                data_key: config.data_key.clone().filter(|k| !k.is_empty()),
                fee_bps: config.fee_bps,
                fee_ua: config.fee_ua.clone().filter(|u| !u.is_empty()),
                admin_token_sha256: config.admin_token.as_deref().filter(|t| !t.is_empty()).map(
                    |t| {
                        use sha2::Digest;
                        sha2::Sha256::digest(t.as_bytes()).into()
                    },
                ),
            },
        );
    if let Some(height) = config.dev_chain_height {
        tracing::warn!(
            height,
            "using SF_DEV_CHAIN_HEIGHT; the scanner will replace this (task 2.5)"
        );
        state.set_chain_tip(height);
    }

    tokio::spawn(auth::purge_nonces(state.db.clone()));
    tokio::spawn(closer::run(state.clone()));
    let network = match config.network.as_str() {
        "mainnet" => Network::Mainnet,
        "regtest" => Network::Regtest,
        _ => Network::Testnet,
    };
    // The scanner keeps the chain tip and every live escrow current. With
    // SF_DEV_CHAIN_HEIGHT set (no node available), it stays off.
    if config.dev_chain_height.is_none() {
        tokio::spawn(scanner::run(
            state.clone(),
            config.zaino_url.clone(),
            network,
            config.wallet_dir.clone(),
        ));
    }
    anyhow::ensure!(
        sealforge_core::fee::fee_rate_allowed(config.fee_bps),
        "SF_FEE_BPS must be 0 (no fee) or from {} to {}",
        sealforge_core::fee::MIN_FEE_BPS,
        sealforge_core::fee::MAX_FEE_BPS
    );
    if let Some(token) = config.admin_token.as_deref().filter(|t| !t.is_empty()) {
        anyhow::ensure!(
            token.len() >= 32,
            "SF_ADMIN_TOKEN must be at least 32 characters"
        );
    }
    if config.fee_bps > 0 {
        let ua = state
            .settings
            .fee_ua
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("SF_FEE_BPS is set, so SF_FEE_UA is required"))?;
        sealforge_coordinator::validate::shielded_ua(ua, network)
            .map_err(|e| anyhow::anyhow!("SF_FEE_UA: {e:?}"))?;
        tracing::info!(bps = config.fee_bps, "protocol fee on new auctions");
    }
    let policy = config.refund_policy()?;
    if let Some(blocks) = config.min_window_blocks {
        anyhow::ensure!(
            config.network != "mainnet",
            "SF_MIN_WINDOW_BLOCKS is not allowed on mainnet"
        );
        sealforge_coordinator::validate::set_min_window_blocks(blocks);
        tracing::warn!(blocks, "shortened minimum auction window (test network)");
    }
    if let Some(notes) = config.max_notes_per_tx {
        anyhow::ensure!(
            config.network != "mainnet",
            "SF_MAX_NOTES_PER_TX is not allowed on mainnet"
        );
        scanner::set_max_notes_per_tx(notes);
        tracing::warn!(
            notes = scanner::max_notes_per_tx(),
            "lowered escrow notes per transaction (test network)"
        );
    }
    if config.refund_delay_secs.is_some() {
        tracing::warn!(
            min = policy.min_delay_secs,
            max = policy.max_delay_secs,
            "shortened refund delays (test network)"
        );
    }
    tokio::spawn(refunds::run(state.clone(), config.refunds_paused, policy));
    tokio::spawn(retention::run(state.db.clone(), config.wallet_dir.clone()));

    // Per-client limits run first, so a refused request never waits for a slot
    // under the in-flight cap (#58).
    let limits = config.rate_limits()?;
    anyhow::ensure!(
        (1..20).contains(&config.http_max_in_flight),
        "SF_HTTP_MAX_IN_FLIGHT must be from 1 to 19, below the database pool size"
    );
    let limiter = std::sync::Arc::new(ratelimit::Limiter::new(&limits));
    tokio::spawn(ratelimit::run_pruner(limiter.clone()));
    let app = routes::router(state)
        .layer(tower::limit::ConcurrencyLimitLayer::new(
            config.http_max_in_flight,
        ))
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            ratelimit::limit,
        ));
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!(
        addr = %config.listen_addr,
        network = %config.network,
        reads_per_min = limits.reads_per_minute,
        writes_per_min = limits.writes_per_minute,
        "coordinator listening"
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
