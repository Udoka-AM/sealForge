mod auctions;
pub mod batches;
mod dkg;
mod organisers;
mod results;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::trace::TraceLayer;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/chain", get(chain))
        .route("/readyz", get(readyz))
        .route("/v1/organisers", post(organisers::register))
        .route("/v1/organisers/me", get(organisers::me))
        .route(
            "/v1/auctions",
            post(auctions::create).get(auctions::list_public),
        )
        .route(
            "/v1/auctions/{code}",
            get(auctions::get_public)
                .patch(auctions::update)
                .delete(auctions::delete_draft),
        )
        .route("/v1/auctions/{code}/open", post(auctions::open))
        .route("/v1/auctions/{code}/cancel", post(auctions::cancel))
        .route("/v1/console/auctions", get(auctions::list_own))
        .route("/v1/console/auctions/{code}", get(auctions::get_own))
        .route(
            "/v1/console/auctions/{code}/batches",
            get(batches::list_for_organiser),
        )
        .route("/v1/trustees/join", post(dkg::join))
        .route("/v1/auctions/{code}/dkg", get(dkg::state))
        .route("/v1/auctions/{code}/dkg/round1", post(dkg::round1))
        .route("/v1/auctions/{code}/dkg/round2", post(dkg::round2))
        .route("/v1/auctions/{code}/dkg/complete", post(dkg::complete))
        .route("/v1/auctions/{code}/publish", post(auctions::publish))
        .route("/v1/auctions/{code}/result", get(results::result))
        .route("/v1/auctions/{code}/refunds", get(results::refund_progress))
        .route("/v1/auctions/{code}/batches", get(batches::list))
        .route("/v1/batches/{id}/commitments", post(batches::commit))
        .route("/v1/batches/{id}/shares", post(batches::share))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn chain(State(state): State<AppState>) -> Json<sealforge_api::types::ChainView> {
    Json(sealforge_api::types::ChainView {
        tip: state.chain_tip(),
    })
}

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Readiness for the uptime monitor: database, lightwallet server (the
/// scanner's tip), scanner lag on live escrows, and failed escrow batches.
/// 503 when anything needs attention; the body says what.
async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    let tip = state.chain_tip();
    let (lowest_scanned, failed): (Option<i32>, i64) = if db_ok {
        sqlx::query_as(
            "SELECT (SELECT min(s.last_height) FROM scanner_state s JOIN auctions a ON a.id = s.auction_id \
                     WHERE a.status IN ('ready', 'open', 'closed', 'published', 'refunding')), \
                    (SELECT count(*) FROM tx_batches WHERE status = 'failed')",
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or((None, 0))
    } else {
        (None, 0)
    };
    let lag = match (tip, lowest_scanned) {
        (Some(t), Some(h)) => Some(t.saturating_sub(h.max(0) as u32)),
        _ => None,
    };
    let ok = db_ok && tip.is_some() && lag.is_none_or(|l| l <= MAX_SCANNER_LAG) && failed == 0;
    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "db": if db_ok { "ok" } else { "down" },
            "chain_tip": tip,
            "scanner_lag_blocks": lag,
            "failed_batches": failed,
        })),
    )
}

/// Blocks a live escrow's scan may trail the tip before readiness fails.
const MAX_SCANNER_LAG: u32 = 4;
