//! Closing auctions at their close height (task 3.3).
//!
//! An auction closes once the scanner has processed `close_height` plus
//! [`CLOSE_CONFIRMATIONS`] blocks, so no in-range bid can still appear and a
//! shallow reorg can't change the bid set. Closing classifies every payment
//! with `sealforge_core::selection` (the same rules the auditor uses), runs
//! `compute_result`, and stores outcomes for refunds.

use sealforge_api::types::AuctionStatus;
use sealforge_core::{
    clearing::{AuctionMode as CoreMode, AuctionRules},
    memo::BidMemo,
    merkle::leaf_hash,
    result::compute_result,
    selection::{classify, included_bids, BidClass, BidWindow, Payment},
    Zatoshi,
};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    db::{self, AuctionRow},
    error::ApiError,
    AppState,
};

/// Blocks after the close height before results are computed (~4 min on NU7).
pub const CLOSE_CONFIRMATIONS: u32 = 10;

#[derive(Debug, FromRow)]
struct BidRow {
    id: Uuid,
    txid: Vec<u8>,
    output_index: i32,
    block_height: i32,
    amount_zat: i64,
    commitment: Option<Vec<u8>>,
    invalid_reason: Option<String>,
}

/// Rebuild the auditor's view of a stored payment. The scanner stores the
/// commitment of every parseable memo and marks memo-level problems
/// (`bad_memo`, `wrong_auction`); everything else is reclassified here.
fn payment(auction_code: &str, row: &BidRow) -> Payment {
    let memo_ok = !matches!(
        row.invalid_reason.as_deref(),
        Some("bad_memo" | "wrong_auction")
    );
    let memo = match (&row.commitment, memo_ok) {
        (Some(c), true) => c.as_slice().try_into().ok().map(|commitment| BidMemo {
            auction_code: auction_code.to_owned(),
            commitment,
            refund_ua: String::new(),
        }),
        _ => None,
    };
    Payment {
        txid: row.txid.as_slice().try_into().unwrap_or([0; 32]),
        output_index: row.output_index as u32,
        block_height: row.block_height as u32,
        amount: Zatoshi(row.amount_zat as u64),
        memo,
    }
}

fn db_status(class: BidClass) -> (&'static str, Option<&'static str>) {
    match class {
        BidClass::Valid => ("valid", None),
        BidClass::UnderReserve => ("under_reserve", None),
        BidClass::Late => ("late", None),
        BidClass::Invalid(reason) => ("invalid", Some(reason.as_str())),
    }
}

/// Close one auction. Returns false if it isn't due yet.
pub async fn close_auction(state: &AppState, auction: &AuctionRow) -> Result<bool, ApiError> {
    if auction.status != AuctionStatus::Open {
        return Ok(false);
    }
    let close_height = auction.close_height as u32;
    let scanned: Option<i32> =
        sqlx::query_scalar("SELECT last_height FROM scanner_state WHERE auction_id = $1")
            .bind(auction.id)
            .fetch_optional(&state.db)
            .await?;
    match scanned {
        Some(h) if h as u32 >= close_height + CLOSE_CONFIRMATIONS => {}
        _ => return Ok(false),
    }
    let open_height = auction
        .open_height
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("open auction without open height")))?
        as u32;

    let mut tx = state.db.begin().await?;
    let rows: Vec<BidRow> = sqlx::query_as(
        "SELECT id, txid, output_index, block_height, amount_zat, commitment, invalid_reason \
         FROM bids WHERE auction_id = $1 FOR UPDATE",
    )
    .bind(auction.id)
    .fetch_all(&mut *tx)
    .await?;

    let window = BidWindow {
        auction_code: auction.code.clone(),
        open_height,
        close_height,
        reserve: Zatoshi(auction.reserve_zat as u64),
    };
    let payments: Vec<Payment> = rows.iter().map(|r| payment(&auction.code, r)).collect();
    let classes = classify(&window, &payments);
    let included = included_bids(&window, &payments);
    let rules = AuctionRules {
        mode: match auction.mode {
            sealforge_api::types::AuctionMode::UniformPrice => CoreMode::UniformPrice,
            sealforge_api::types::AuctionMode::FirstPrice => CoreMode::FirstPrice,
        },
        quantity: auction.quantity as u32,
        reserve: Zatoshi(auction.reserve_zat as u64),
    };
    let result = compute_result(&rules, &included).map_err(|e| ApiError::Internal(e.into()))?;
    let outcomes: std::collections::BTreeMap<[u8; 32], _> = result
        .clearing
        .outcomes
        .iter()
        .map(|o| (o.leaf_hash, o))
        .collect();

    for ((row, class), payment) in rows.iter().zip(&classes).zip(&payments) {
        let (status, reason) = db_status(*class);
        // Memo-level problems were judged by the scanner from the raw memo; keep its reason.
        let scanner_reason = row
            .invalid_reason
            .as_deref()
            .filter(|r| matches!(*r, "bad_memo" | "wrong_auction"));
        let reason = scanner_reason.or(reason);
        // Included bids get their leaf and outcome; everything else is refunded in full.
        let (leaf, is_winner, price_paid, refund_due) = if class.is_included() {
            let commitment = payment
                .memo
                .as_ref()
                .expect("included bids have a memo")
                .commitment;
            let leaf = leaf_hash(
                &payment.txid,
                payment.output_index,
                payment.amount.0,
                &commitment,
            );
            let outcome = outcomes[&leaf];
            (
                Some(leaf.to_vec()),
                Some(outcome.is_winner),
                outcome.price_paid.0,
                outcome.refund_due.0,
            )
        } else {
            (None, Some(false), 0, row.amount_zat as u64)
        };
        sqlx::query(
            "UPDATE bids SET status = $1::bid_status, invalid_reason = $2, leaf_hash = $3, is_winner = $4, \
             price_paid_zat = $5, refund_due_zat = $6 WHERE id = $7",
        )
        .bind(status)
        .bind(reason)
        .bind(leaf)
        .bind(is_winner)
        .bind(price_paid as i64)
        .bind(refund_due as i64)
        .bind(row.id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO clearing_results (auction_id, algorithm_version, clearing_price_zat, total_volume_zat, \
         valid_bid_count, winner_count, merkle_root, scan_from_height, scan_to_height) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(auction.id)
    .bind(&result.algorithm_version)
    .bind(result.clearing.clearing_price.map(|p| p.0 as i64))
    .bind(result.clearing.total_volume.0 as i64)
    .bind(result.clearing.valid_bid_count as i32)
    .bind(result.clearing.winner_count as i32)
    .bind(&result.merkle_root[..])
    .bind(open_height as i32)
    .bind(close_height as i32)
    .execute(&mut *tx)
    .await?;

    let moved = sqlx::query("UPDATE auctions SET status = 'closed', updated_at = now() WHERE id = $1 AND status = 'open'")
        .bind(auction.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if moved == 0 {
        return Ok(false);
    }
    db::audit(
        &mut *tx,
        Some(auction.id),
        "system",
        None,
        "auction.transition",
        json!({ "from": "open", "to": "closed", "merkle_root": hex::encode(result.merkle_root) }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(code = %auction.code, bids = rows.len(), "auction closed");
    Ok(true)
}

/// Close every open auction that is due. Returns the codes closed.
pub async fn close_due(state: &AppState) -> Result<Vec<String>, ApiError> {
    let mut closed = Vec::new();
    for auction in db::auctions_where_status(&state.db, &[AuctionStatus::Open]).await? {
        if close_auction(state, &auction).await? {
            closed.push(auction.code);
        }
    }
    Ok(closed)
}

/// Background loop: check for due auctions every block (~25 s on NU7).
pub async fn run(state: AppState) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(25));
    loop {
        tick.tick().await;
        if let Err(err) = close_due(&state).await {
            tracing::warn!(error = %err, "closing auctions failed");
        }
    }
}
