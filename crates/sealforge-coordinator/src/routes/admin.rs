//! The operator's admin endpoints, behind `SF_ADMIN_TOKEN` (a bearer token).
//! Off when the token isn't set. Today there is one: setting an auction's
//! protocol fee rate before it opens.

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap},
    Json,
};
use sealforge_api::types::AuctionView;
use sealforge_core::fee::{fee_rate_allowed, MAX_FEE_BPS, MIN_FEE_BPS};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{db, error::ApiError, lifecycle::is_editable, validate, AppState};

/// Check the bearer token. Only SHA-256 digests are compared, so the
/// comparison's timing tells nothing about the token.
fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = state.settings.admin_token_sha256 else {
        return Err(ApiError::NotFound);
    };
    let given = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::Unauthorized("admin token required".into()))?;
    let digest: [u8; 32] = Sha256::digest(given.trim().as_bytes()).into();
    if digest != expected {
        return Err(ApiError::Unauthorized("admin token required".into()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetFee {
    /// Basis points: 0 for no fee, or 500 to 1500 (5% to 15%).
    pub fee_bps: u16,
}

/// `PUT /v1/admin/auctions/{code}/fee`: set an auction's protocol fee rate.
/// Only before the auction opens, so bidders always see the final terms. The
/// fee is paid to the coordinator's `SF_FEE_UA`.
pub async fn set_fee(
    State(state): State<AppState>,
    Path(code): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SetFee>,
) -> Result<Json<AuctionView>, ApiError> {
    require_admin(&state, &headers)?;
    if !fee_rate_allowed(input.fee_bps) {
        return Err(ApiError::Validation(format!(
            "fee_bps must be 0 (no fee) or from {MIN_FEE_BPS} to {MAX_FEE_BPS}"
        )));
    }
    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    if !is_editable(row.status) {
        return Err(ApiError::conflict(
            "not_editable",
            "the fee can only be set before the auction opens",
        ));
    }
    let fee_ua = if input.fee_bps == 0 {
        None
    } else {
        let ua = state.settings.fee_ua.as_deref().ok_or_else(|| {
            ApiError::Unavailable("SF_FEE_UA isn't set, so no fee can be charged".into())
        })?;
        validate::shielded_ua(ua, row.network).map_err(|_| {
            ApiError::Unavailable(format!(
                "SF_FEE_UA isn't a shielded address on {:?}",
                row.network
            ))
        })?;
        Some(ua.to_owned())
    };

    let mut tx = state.db.begin().await?;
    let updated = sqlx::query(
        "UPDATE auctions SET fee_bps = $1, fee_ua = $2, updated_at = now() \
         WHERE id = $3 AND status IN ('draft', 'dkg', 'ready')",
    )
    .bind(input.fee_bps as i16)
    .bind(&fee_ua)
    .bind(row.id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated != 1 {
        return Err(ApiError::conflict(
            "not_editable",
            "the auction opened while the fee was being set",
        ));
    }
    db::audit(
        &mut *tx,
        Some(row.id),
        "admin",
        None,
        "auction.fee_set",
        json!({ "fee_bps": input.fee_bps, "previous_bps": row.fee_bps }),
    )
    .await?;
    tx.commit().await?;

    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(db::auction_view(&state.db, &row, true).await?))
}
