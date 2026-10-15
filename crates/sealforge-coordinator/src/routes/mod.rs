mod auctions;
mod batches;
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

async fn readyz(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    // TODO(2.5): include Zaino reachability and scanner lag.
    Ok(Json(json!({ "db": "ok", "chain_tip": state.chain_tip() })))
}
