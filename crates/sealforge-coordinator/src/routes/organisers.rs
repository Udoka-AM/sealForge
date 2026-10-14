use axum::{extract::State, http::StatusCode, Json};
use sealforge_api::types::{OrganiserView, RegisterOrganiser};
use uuid::Uuid;

use crate::{auth::Signed, db, error::ApiError, AppState};

/// `POST /v1/organisers`: register the signing key as an organiser (invite only).
pub async fn register(
    State(state): State<AppState>,
    req: Signed,
) -> Result<(StatusCode, Json<OrganiserView>), ApiError> {
    let input: RegisterOrganiser = req.json()?;
    let name = input.display_name.trim();
    if !(1..=80).contains(&name.chars().count()) {
        return Err(ApiError::Validation(
            "display name must be 1–80 characters".into(),
        ));
    }
    if !state
        .settings
        .invite_codes
        .iter()
        .any(|c| c == &input.invite_code)
    {
        return Err(ApiError::Forbidden);
    }
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO organisers (display_name, pubkey) VALUES ($1, $2) ON CONFLICT (pubkey) DO NOTHING RETURNING id",
    )
    .bind(name)
    .bind(&req.pubkey[..])
    .fetch_optional(&state.db)
    .await?;
    let id = id.ok_or_else(|| {
        ApiError::conflict("already_registered", "this key is already registered")
    })?;
    db::audit(
        &state.db,
        None,
        "organiser",
        Some(id),
        "organiser.registered",
        serde_json::json!({}),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(OrganiserView {
            id: id.to_string(),
            display_name: name.to_owned(),
        }),
    ))
}

/// `GET /v1/organisers/me`
pub async fn me(
    State(state): State<AppState>,
    req: Signed,
) -> Result<Json<OrganiserView>, ApiError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, display_name FROM organisers WHERE pubkey = $1 AND disabled_at IS NULL",
    )
    .bind(&req.pubkey[..])
    .fetch_optional(&state.db)
    .await?;
    let (id, display_name) = row.ok_or(ApiError::Forbidden)?;
    Ok(Json(OrganiserView {
        id: id.to_string(),
        display_name,
    }))
}
