//! Row types and queries. Queries are checked at runtime and covered by the
//! integration tests in `tests/api.rs`.

use sealforge_api::types::{
    AuctionMode, AuctionStatus, AuctionView, BidMode, DkgStatus, Network, OwnerFields, TrusteeView,
};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, FromRow)]
pub struct AuctionRow {
    pub id: Uuid,
    pub code: String,
    pub organiser_id: Uuid,
    pub title: String,
    pub description: String,
    pub lot_image_url: Option<String>,
    pub network: Network,
    pub mode: AuctionMode,
    pub bid_mode: BidMode,
    pub quantity: i32,
    pub reserve_zat: i64,
    pub open_height: Option<i32>,
    pub close_height: i32,
    pub reveal_close_height: Option<i32>,
    pub escrow_ua: Option<String>,
    pub payout_ua: String,
    pub fee_buffer_zat: i64,
    pub status: AuctionStatus,
}

/// Column list for `AuctionRow`, spliced in at compile time so queries stay static strings.
macro_rules! auction_columns {
    () => {
        "id, code, organiser_id, title, description, lot_image_url, network, mode, \
     bid_mode, quantity, reserve_zat, open_height, close_height, reveal_close_height, escrow_ua, payout_ua, \
     fee_buffer_zat, status"
    };
}

#[derive(Debug, FromRow)]
struct TrusteeRow {
    slot: i16,
    label: String,
    dkg_status: DkgStatus,
}

pub async fn auction_by_code(
    db: impl PgExecutor<'_>,
    code: &str,
) -> Result<Option<AuctionRow>, ApiError> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        auction_columns!(),
        " FROM auctions WHERE code = $1"
    ))
    .bind(code)
    .fetch_optional(db)
    .await?)
}

pub async fn auctions_where_status(
    db: &PgPool,
    statuses: &[AuctionStatus],
) -> Result<Vec<AuctionRow>, ApiError> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        auction_columns!(),
        " FROM auctions WHERE status = ANY($1) ORDER BY created_at DESC LIMIT 200"
    ))
    .bind(statuses)
    .fetch_all(db)
    .await?)
}

pub async fn auctions_for_organiser(
    db: &PgPool,
    organiser_id: Uuid,
) -> Result<Vec<AuctionRow>, ApiError> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        auction_columns!(),
        " FROM auctions WHERE organiser_id = $1 ORDER BY created_at DESC"
    ))
    .bind(organiser_id)
    .fetch_all(db)
    .await?)
}

/// Build the API view. `owner` adds fields only the organiser may see.
pub async fn auction_view(
    db: &PgPool,
    row: &AuctionRow,
    owner: bool,
) -> Result<AuctionView, ApiError> {
    let trustees: Vec<TrusteeRow> = sqlx::query_as(
        "SELECT slot, label, dkg_status FROM trustees WHERE auction_id = $1 ORDER BY slot",
    )
    .bind(row.id)
    .fetch_all(db)
    .await?;
    let bids: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM bids WHERE auction_id = $1 \
         AND coalesce(invalid_reason, '') NOT IN ('fee_buffer', 'reveal')",
    )
    .bind(row.id)
    .fetch_one(db)
    .await?;
    let bids = bids as u32;
    Ok(AuctionView {
        code: row.code.clone(),
        title: row.title.clone(),
        description: row.description.clone(),
        lot_image_url: row.lot_image_url.clone(),
        network: row.network,
        mode: row.mode,
        bid_mode: row.bid_mode,
        quantity: row.quantity as u32,
        reserve_zat: row.reserve_zat as u64,
        open_height: row.open_height.map(|h| h as u32),
        close_height: row.close_height as u32,
        reveal_close_height: row.reveal_close_height.map(|h| h as u32),
        status: row.status,
        escrow_ua: row.escrow_ua.clone(),
        trustees: trustees
            .into_iter()
            .map(|t| TrusteeView {
                slot: t.slot as u16,
                label: t.label,
                dkg_status: t.dkg_status,
            })
            .collect(),
        bid_count: (owner || bids >= sealforge_api::types::MIN_PUBLIC_BID_COUNT).then_some(bids),
        owner: owner.then(|| OwnerFields {
            payout_ua: row.payout_ua.clone(),
            fee_buffer_zat: row.fee_buffer_zat as u64,
        }),
    })
}

/// Append to the audit log (never request bodies or addresses).
pub async fn audit(
    db: impl PgExecutor<'_>,
    auction_id: Option<Uuid>,
    actor_kind: &str,
    actor_id: Option<Uuid>,
    action: &str,
    detail: serde_json::Value,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO audit_log (auction_id, actor_kind, actor_id, action, detail) VALUES ($1, $2, $3, $4, $5)")
        .bind(auction_id)
        .bind(actor_kind)
        .bind(actor_id)
        .bind(action)
        .bind(detail)
        .execute(db)
        .await?;
    Ok(())
}
