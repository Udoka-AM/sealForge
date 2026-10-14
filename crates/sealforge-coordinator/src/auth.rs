//! `SF-Sig` request authentication (task 2.2). The signing scheme lives in
//! `sealforge_api::auth`; this module adds the server clock, nonce replay
//! protection and role lookups.

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::header::AUTHORIZATION,
};
use sealforge_api::auth::{parse_header, verify_request, AuthError};
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::ApiError, AppState};

/// A request whose signature and nonce have been verified.
#[derive(Debug)]
pub struct Signed {
    pub pubkey: [u8; 32],
    pub body: Bytes,
}

impl Signed {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, ApiError> {
        serde_json::from_slice(&self.body)
            .map_err(|e| ApiError::BadRequest(format!("invalid JSON body: {e}")))
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

impl FromRequest<AppState> for Signed {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("missing SF-Sig authorization".into()))?;
        let parsed = parse_header(header).map_err(|e| ApiError::Unauthorized(e.to_string()))?;
        let method = req.method().as_str().to_owned();
        let path = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str().to_owned())
            .unwrap_or_else(|| "/".into());
        let body = Bytes::from_request(req, state)
            .await
            .map_err(|_| ApiError::BadRequest("unreadable request body".into()))?;

        verify_request(&parsed, &method, &path, &body, unix_now()).map_err(|e| match e {
            AuthError::StaleTimestamp => {
                ApiError::Unauthorized("request timestamp too old or too new".into())
            }
            other => ApiError::Unauthorized(other.to_string()),
        })?;

        let fresh = sqlx::query(
            "INSERT INTO api_nonces (pubkey, nonce) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(&parsed.key[..])
        .bind(&parsed.nonce[..])
        .execute(&state.db)
        .await?
        .rows_affected();
        if fresh == 0 {
            return Err(ApiError::Unauthorized("nonce already used".into()));
        }

        Ok(Self {
            pubkey: parsed.key,
            body,
        })
    }
}

/// The registered, enabled organiser behind a signed request.
pub async fn organiser_id(db: &PgPool, pubkey: &[u8; 32]) -> Result<Uuid, ApiError> {
    sqlx::query_scalar("SELECT id FROM organisers WHERE pubkey = $1 AND disabled_at IS NULL")
        .bind(&pubkey[..])
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::Forbidden)
}

/// Delete nonces older than the replay window, every minute.
pub async fn purge_nonces(db: PgPool) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        tick.tick().await;
        if let Err(err) =
            sqlx::query("DELETE FROM api_nonces WHERE seen_at < now() - interval '10 minutes'")
                .execute(&db)
                .await
        {
            tracing::warn!(error = %err, "nonce purge failed");
        }
    }
}
