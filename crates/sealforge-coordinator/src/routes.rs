use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        // TODO(2.3): /v1/auctions CRUD + lifecycle transitions
        // TODO(2.4): /v1/auctions/{id}/dkg/*
        // TODO(3.3): /v1/auctions/{id}/close, /publish, /result
        // TODO(3.9): /v1/batches/*
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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
    Ok(Json(json!({ "db": "ok" })))
}
