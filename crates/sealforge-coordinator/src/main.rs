//! sealForge coordinator: REST API, escrow scanner, refund scheduler.
//! See docs/02-TRD.md §2.3 and §2.6.

mod config;
mod routes;

use anyhow::Context;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

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

    // TODO(2.5): spawn the escrow scanner task.
    // TODO(3.9): spawn the refund scheduler task (respect SF_REFUNDS_PAUSED).

    let app = routes::router(routes::AppState { db });
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, network = %config.network, "coordinator listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
