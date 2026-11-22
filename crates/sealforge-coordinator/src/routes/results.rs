//! Public results.

use axum::{
    extract::{Path, State},
    Json,
};
use sealforge_api::types::{
    AuctionStatus, BidMode, PublishedResult, ResultBid, ResultRules, TrusteeAttestation,
};
use sqlx::FromRow;

use crate::{db, error::ApiError, refunds, AppState};

#[derive(Debug, FromRow)]
struct ResultRow {
    algorithm_version: String,
    clearing_price_zat: Option<i64>,
    total_volume_zat: i64,
    valid_bid_count: i32,
    winner_count: i32,
    merkle_root: Vec<u8>,
    scan_from_height: i32,
    scan_to_height: i32,
}

#[derive(Debug, FromRow)]
struct LeafRow {
    txid: Vec<u8>,
    output_index: i32,
    block_height: i32,
    amount_zat: i64,
    reveal_price_zat: Option<i64>,
    commitment: Option<Vec<u8>>,
    leaf_hash: Vec<u8>,
    is_winner: Option<bool>,
    price_paid_zat: Option<i64>,
    refund_due_zat: Option<i64>,
}

/// `GET /v1/auctions/{code}/result`: 404 until the organiser publishes.
pub async fn result(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<PublishedResult>, ApiError> {
    let auction = db::auction_by_code(&state.db, &code)
        .await?
        .filter(|a| {
            matches!(
                a.status,
                AuctionStatus::Published | AuctionStatus::Refunding | AuctionStatus::Settled
            )
        })
        .ok_or(ApiError::NotFound)?;
    let uivk: Option<String> = sqlx::query_scalar("SELECT escrow_ivk FROM auctions WHERE id = $1")
        .bind(auction.id)
        .fetch_one(&state.db)
        .await?;
    let trustees = db::trustees_public(&state.db, auction.id).await?;
    let r: ResultRow = sqlx::query_as(
        "SELECT algorithm_version, clearing_price_zat, total_volume_zat, valid_bid_count, winner_count, \
         merkle_root, scan_from_height, scan_to_height FROM clearing_results WHERE auction_id = $1",
    )
    .bind(auction.id)
    .fetch_one(&state.db)
    .await?;
    let leaves: Vec<LeafRow> = sqlx::query_as(
        "SELECT txid, output_index, block_height, amount_zat, reveal_price_zat, commitment, leaf_hash, is_winner, price_paid_zat, \
         refund_due_zat FROM bids \
         WHERE auction_id = $1 AND leaf_hash IS NOT NULL ORDER BY leaf_hash",
    )
    .bind(auction.id)
    .fetch_all(&state.db)
    .await?;

    // The protocol fee follows from the published bids alone (issue #74).
    let proceeds: u64 = leaves
        .iter()
        .map(|l| l.price_paid_zat.unwrap_or(0).max(0) as u64)
        .sum();
    let fee_bps = auction.fee_bps.max(0) as u16;
    let fee_zat = sealforge_core::fee::protocol_fee_zat(proceeds, fee_bps);
    Ok(Json(PublishedResult {
        fee_bps,
        fee_ua: (fee_bps > 0).then(|| auction.fee_ua.clone()).flatten(),
        fee_zat,
        auction_code: auction.code.clone(),
        network: auction.network,
        rules: ResultRules {
            mode: auction.mode,
            quantity: auction.quantity as u32,
            reserve_zat: auction.reserve_zat as u64,
            bid_mode: auction.bid_mode,
            close_height: (auction.bid_mode == BidMode::CommitReveal)
                .then_some(auction.close_height as u32),
        },
        scan_from_height: r.scan_from_height as u32,
        scan_to_height: r.scan_to_height as u32,
        uivk: uivk.unwrap_or_default(),
        algorithm_version: r.algorithm_version,
        clearing_price_zat: r.clearing_price_zat.map(|p| p as u64),
        total_volume_zat: r.total_volume_zat as u64,
        valid_bid_count: r.valid_bid_count as u32,
        winner_count: r.winner_count as u32,
        merkle_root: hex::encode(r.merkle_root),
        bids: leaves
            .into_iter()
            .map(|l| ResultBid {
                txid: hex::encode(l.txid),
                output_index: l.output_index as u32,
                block_height: l.block_height as u32,
                // Commit-reveal: the bid is the revealed price; the payment was the deposit.
                amount_zat: l.reveal_price_zat.unwrap_or(l.amount_zat) as u64,
                deposit_zat: l.reveal_price_zat.map(|_| l.amount_zat as u64),
                // The closer set refund = deposit + reveal - paid.
                reveal_zat: l.reveal_price_zat.map(|_| {
                    (l.refund_due_zat.unwrap_or(0) + l.price_paid_zat.unwrap_or(0) - l.amount_zat)
                        as u64
                }),
                commitment: hex::encode(l.commitment.unwrap_or_default()),
                leaf_hash: hex::encode(l.leaf_hash),
                is_winner: l.is_winner.unwrap_or(false),
                price_paid_zat: l.price_paid_zat.unwrap_or(0) as u64,
                refund_due_zat: l.refund_due_zat.unwrap_or(0) as u64,
            })
            .collect(),
        escrow_ua: auction.escrow_ua.clone(),
        trustees: trustees
            .into_iter()
            .map(|t| TrusteeAttestation {
                identity: t.identity(),
                slot: t.slot as u16,
                label: t.label,
                pubkey: t.pubkey.map(hex::encode),
                signature: t.escrow_attestation.map(hex::encode),
            })
            .collect(),
    }))
}

/// `GET /v1/auctions/{code}/refunds`: aggregate refund progress (no per-bid data).
pub async fn refund_progress(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<refunds::RefundProgress>, ApiError> {
    let auction = db::auction_by_code(&state.db, &code)
        .await?
        .filter(|a| {
            matches!(
                a.status,
                AuctionStatus::Published | AuctionStatus::Refunding | AuctionStatus::Settled
            )
        })
        .ok_or(ApiError::NotFound)?;
    Ok(Json(refunds::progress(&state, auction.id).await?))
}
