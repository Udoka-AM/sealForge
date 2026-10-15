//! Auction CRUD and organiser-driven lifecycle transitions (task 2.3).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use sealforge_api::types::{
    AuctionStatus, AuctionView, CreateAuction, CreatedAuction, TrusteeInvite, UpdateAuction,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::{organiser_id, Signed},
    codes,
    db::{self, AuctionRow},
    error::ApiError,
    lifecycle::{can_transition, is_editable, is_public, Actor},
    validate::{self, MIN_OPEN_BLOCKS},
    AppState,
};

/// Load an auction owned by the signing organiser. Other organisers get 404,
/// so auction codes can't be probed.
async fn owned_auction(
    state: &AppState,
    req: &Signed,
    code: &str,
) -> Result<(Uuid, AuctionRow), ApiError> {
    let organiser = organiser_id(&state.db, &req.pubkey).await?;
    let row = db::auction_by_code(&state.db, code)
        .await?
        .filter(|a| a.organiser_id == organiser)
        .ok_or(ApiError::NotFound)?;
    Ok((organiser, row))
}

fn invalid_transition(from: AuctionStatus, to: AuctionStatus) -> ApiError {
    ApiError::conflict(
        "invalid_transition",
        format!("cannot move an auction from {from:?} to {to:?}").to_lowercase(),
    )
}

/// Move `row` to `to`, guarding against concurrent changes.
async fn transition(
    state: &AppState,
    row: &AuctionRow,
    to: AuctionStatus,
    actor: Actor,
    actor_id: Uuid,
    open_height: Option<u32>,
) -> Result<(), ApiError> {
    if !can_transition(row.status, to, actor) {
        return Err(invalid_transition(row.status, to));
    }
    let mut tx = state.db.begin().await?;
    let updated = sqlx::query(
        "UPDATE auctions SET status = $1, open_height = COALESCE($2, open_height), updated_at = now() \
         WHERE id = $3 AND status = $4",
    )
    .bind(to)
    .bind(open_height.map(|h| h as i32))
    .bind(row.id)
    .bind(row.status)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(ApiError::conflict(
            "status_changed",
            "the auction changed; reload and retry",
        ));
    }
    db::audit(
        &mut *tx,
        Some(row.id),
        "organiser",
        Some(actor_id),
        "auction.transition",
        json!({ "from": row.status, "to": to }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// `POST /v1/auctions`: create a draft with three trustee slots.
pub async fn create(
    State(state): State<AppState>,
    req: Signed,
) -> Result<(StatusCode, Json<CreatedAuction>), ApiError> {
    let organiser = organiser_id(&state.db, &req.pubkey).await?;
    let input: CreateAuction = req.json()?;
    validate::create_auction(&input)?;

    let mut tx = state.db.begin().await?;
    // Retry on the (astronomically unlikely) code collision.
    let mut created = None;
    for _ in 0..5 {
        let code = codes::auction_code();
        let id: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO auctions (code, organiser_id, title, description, lot_image_url, network, mode, bid_mode, \
             quantity, reserve_zat, close_height, payout_ua, reveal_close_height) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
             ON CONFLICT (code) DO NOTHING RETURNING id",
        )
        .bind(&code)
        .bind(organiser)
        .bind(input.title.trim())
        .bind(&input.description)
        .bind(&input.lot_image_url)
        .bind(input.network)
        .bind(input.mode)
        .bind(input.bid_mode)
        .bind(input.quantity as i32)
        .bind(input.reserve_zat as i64)
        .bind(input.close_height as i32)
        .bind(&input.payout_ua)
        .bind(input.reveal_close_height.map(|h| h as i32))
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = id {
            created = Some((id, code));
            break;
        }
    }
    let (id, code) = created
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("could not allocate an auction code")))?;

    let mut invites = Vec::with_capacity(input.trustees.len());
    for trustee in &input.trustees {
        let invite = codes::invite_code();
        sqlx::query("INSERT INTO trustees (auction_id, slot, label, invite_code_hash) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(trustee.slot as i16)
            .bind(trustee.label.trim())
            .bind(codes::sha256(&invite))
            .execute(&mut *tx)
            .await?;
        invites.push(TrusteeInvite {
            slot: trustee.slot,
            label: trustee.label.trim().to_owned(),
            invite_code: invite,
        });
    }
    db::audit(
        &mut *tx,
        Some(id),
        "organiser",
        Some(organiser),
        "auction.created",
        json!({ "code": code }),
    )
    .await?;
    tx.commit().await?;

    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedAuction {
            auction: db::auction_view(&state.db, &row, true).await?,
            trustee_invites: invites,
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    status: Option<AuctionStatus>,
}

/// `GET /v1/auctions?status=open`: public auctions only.
pub async fn list_public(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<AuctionView>>, ApiError> {
    let statuses: Vec<AuctionStatus> = match query.status {
        Some(s) if is_public(s) => vec![s],
        Some(_) => return Ok(Json(vec![])),
        None => [
            AuctionStatus::Open,
            AuctionStatus::Closed,
            AuctionStatus::Published,
            AuctionStatus::Refunding,
            AuctionStatus::Settled,
        ]
        .to_vec(),
    };
    let mut views = Vec::new();
    for row in db::auctions_where_status(&state.db, &statuses).await? {
        views.push(db::auction_view(&state.db, &row, false).await?);
    }
    Ok(Json(views))
}

/// `GET /v1/auctions/{code}`: public view. Drafts and cancelled auctions are hidden.
pub async fn get_public(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<AuctionView>, ApiError> {
    let row = db::auction_by_code(&state.db, &code)
        .await?
        .filter(|a| is_public(a.status))
        .ok_or(ApiError::NotFound)?;
    Ok(Json(db::auction_view(&state.db, &row, false).await?))
}

/// `GET /v1/console/auctions`: the signing organiser's auctions.
pub async fn list_own(
    State(state): State<AppState>,
    req: Signed,
) -> Result<Json<Vec<AuctionView>>, ApiError> {
    let organiser = organiser_id(&state.db, &req.pubkey).await?;
    let mut views = Vec::new();
    for row in db::auctions_for_organiser(&state.db, organiser).await? {
        views.push(db::auction_view(&state.db, &row, true).await?);
    }
    Ok(Json(views))
}

/// `GET /v1/console/auctions/{code}`
pub async fn get_own(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<AuctionView>, ApiError> {
    let (_, row) = owned_auction(&state, &req, &code).await?;
    Ok(Json(db::auction_view(&state.db, &row, true).await?))
}

/// `PATCH /v1/auctions/{code}`: edit before opening.
pub async fn update(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<AuctionView>, ApiError> {
    let (organiser, row) = owned_auction(&state, &req, &code).await?;
    if !is_editable(row.status) {
        return Err(ApiError::conflict(
            "not_editable",
            "auctions can only be edited before they open",
        ));
    }
    let input: UpdateAuction = req.json()?;
    validate::update_auction(&input, row.network)?;

    let mut tx = state.db.begin().await?;
    let updated = sqlx::query(
        "UPDATE auctions SET \
           title = COALESCE($1, title), description = COALESCE($2, description), \
           lot_image_url = COALESCE($3, lot_image_url), quantity = COALESCE($4, quantity), \
           reserve_zat = COALESCE($5, reserve_zat), close_height = COALESCE($6, close_height), \
           payout_ua = COALESCE($7, payout_ua), updated_at = now() \
         WHERE id = $8 AND status = $9",
    )
    .bind(input.title.as_deref().map(str::trim))
    .bind(&input.description)
    .bind(&input.lot_image_url)
    .bind(input.quantity.map(|q| q as i32))
    .bind(input.reserve_zat.map(|r| r as i64))
    .bind(input.close_height.map(|h| h as i32))
    .bind(&input.payout_ua)
    .bind(row.id)
    .bind(row.status)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(ApiError::conflict(
            "status_changed",
            "the auction changed; reload and retry",
        ));
    }
    db::audit(
        &mut *tx,
        Some(row.id),
        "organiser",
        Some(organiser),
        "auction.updated",
        json!({}),
    )
    .await?;
    tx.commit().await?;

    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(db::auction_view(&state.db, &row, true).await?))
}

/// `POST /v1/auctions/{code}/open`
pub async fn open(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<AuctionView>, ApiError> {
    let (organiser, row) = owned_auction(&state, &req, &code).await?;
    if !can_transition(row.status, AuctionStatus::Open, Actor::Organiser) {
        return Err(invalid_transition(row.status, AuctionStatus::Open));
    }
    if row.escrow_ua.is_none() {
        return Err(ApiError::conflict(
            "dkg_incomplete",
            "the trustees have not finished key generation",
        ));
    }
    // The fee buffer is optional (PRD Q8): fees come out of the escrow and the payout
    // sweep absorbs them. It only matters for an auction that may end with no winners.
    let tip = state
        .chain_tip()
        .ok_or_else(|| ApiError::Unavailable("chain height unknown; is the node synced?".into()))?;
    if (row.close_height as u32) < tip + MIN_OPEN_BLOCKS {
        return Err(ApiError::Validation(format!(
            "close height must be at least {} (current height {tip} + {MIN_OPEN_BLOCKS} blocks)",
            tip + MIN_OPEN_BLOCKS
        )));
    }
    transition(
        &state,
        &row,
        AuctionStatus::Open,
        Actor::Organiser,
        organiser,
        Some(tip),
    )
    .await?;
    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(db::auction_view(&state.db, &row, true).await?))
}

/// `POST /v1/auctions/{code}/cancel`: before opening, or while open with no bids.
pub async fn cancel(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<AuctionView>, ApiError> {
    let (organiser, row) = owned_auction(&state, &req, &code).await?;
    if row.status == AuctionStatus::Open {
        let bids: i64 = sqlx::query_scalar("SELECT count(*) FROM bids WHERE auction_id = $1")
            .bind(row.id)
            .fetch_one(&state.db)
            .await?;
        if bids > 0 {
            return Err(ApiError::conflict(
                "has_bids",
                "an auction with bids can't be cancelled; it closes with full refunds instead",
            ));
        }
    }
    transition(
        &state,
        &row,
        AuctionStatus::Cancelled,
        Actor::Organiser,
        organiser,
        None,
    )
    .await?;
    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(db::auction_view(&state.db, &row, true).await?))
}

/// `DELETE /v1/auctions/{code}`: drafts only.
pub async fn delete_draft(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<StatusCode, ApiError> {
    let (organiser, row) = owned_auction(&state, &req, &code).await?;
    if row.status != AuctionStatus::Draft {
        return Err(ApiError::conflict(
            "not_draft",
            "only drafts can be deleted; cancel instead",
        ));
    }
    let mut tx = state.db.begin().await?;
    let deleted = sqlx::query("DELETE FROM auctions WHERE id = $1 AND status = 'draft'")
        .bind(row.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(ApiError::conflict(
            "status_changed",
            "the auction changed; reload and retry",
        ));
    }
    db::audit(
        &mut *tx,
        None,
        "organiser",
        Some(organiser),
        "auction.deleted",
        json!({ "code": row.code }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /v1/auctions/{code}/publish`: make the result public and reveal the
/// escrow's incoming viewing key so anyone can recompute it (PRD AC6).
pub async fn publish(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<AuctionView>, ApiError> {
    let (organiser, row) = owned_auction(&state, &req, &code).await?;
    if !can_transition(row.status, AuctionStatus::Published, Actor::Organiser) {
        return Err(invalid_transition(row.status, AuctionStatus::Published));
    }
    let data_key = state
        .settings
        .data_key
        .clone()
        .ok_or_else(|| ApiError::Unavailable("SF_DATA_KEY is not configured".into()))?;
    let ufvk: Option<String> = sqlx::query_scalar(
        "SELECT pgp_sym_decrypt(escrow_ufvk_enc, $2) FROM auctions WHERE id = $1",
    )
    .bind(row.id)
    .bind(&data_key)
    .fetch_one(&state.db)
    .await?;
    let ufvk = ufvk
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("auction has no escrow viewing key")))?;
    let params = validate::params_for(row.network);
    let uivk = zcash_keys::keys::UnifiedFullViewingKey::decode(&params, &ufvk)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("stored UFVK does not decode: {e}")))?
        .to_unified_incoming_viewing_key()
        .encode(&params)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("UIVK encoding failed: {e:?}")))?;

    let mut tx = state.db.begin().await?;
    let moved = sqlx::query(
        "UPDATE auctions SET status = 'published', escrow_ivk = $1, published_at = now(), updated_at = now() \
         WHERE id = $2 AND status = 'closed'",
    )
    .bind(&uivk)
    .bind(row.id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if moved == 0 {
        return Err(ApiError::conflict(
            "status_changed",
            "the auction changed; reload and retry",
        ));
    }
    db::audit(
        &mut *tx,
        Some(row.id),
        "organiser",
        Some(organiser),
        "auction.transition",
        json!({ "from": row.status, "to": AuctionStatus::Published }),
    )
    .await?;
    tx.commit().await?;
    let row = db::auction_by_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(db::auction_view(&state.db, &row, true).await?))
}
