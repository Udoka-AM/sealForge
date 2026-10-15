use anyhow::Context;
use clap::Parser;
use sealforge_api::types::Network;
use sealforge_coordinator::{
    auth, closer, config::Config, refunds, routes, scanner, AppState, Settings,
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
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;
    sqlx::migrate!("../../migrations")
        .run(&db)
        .await
        .context("running migrations")?;

    let state = AppState::new(
        db,
        Settings {
            invite_codes: config.invite_codes(),
            data_key: config.data_key.clone().filter(|k| !k.is_empty()),
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
    tokio::spawn(refunds::run(state.clone(), config.refunds_paused));

    let app = routes::router(state);
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, network = %config.network, "coordinator listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
