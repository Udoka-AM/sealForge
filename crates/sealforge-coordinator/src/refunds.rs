//! Refund planning and batching (task 3.9, off-chain half).
//!
//! 1. **Plan** (once per published auction): every bid with a refund of at
//!    least 0.0001 ZEC and a known refund address gets canonical notes, each
//!    with its own CSPRNG delay (`sealforge_core::refunds`). The auction moves
//!    `published → refunding`. Dust below 0.0001 ZEC is not sent.
//! 2. **Batch** (continuously): notes whose delay has passed are shuffled and
//!    grouped into escrow transactions that interleave bidders, as `tx_batches`
//!    in `building`. Building the PCZT and collecting trustee signatures needs
//!    the chain and lands with the scanner (TODO(3.9)).
//!
//! Payments whose memo couldn't be read have no refund address. They are
//! reported for manual review instead of being planned.

use std::time::Duration;

use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, seq::SliceRandom, RngCore};
use sealforge_api::types::AuctionStatus;
use sealforge_core::{
    refunds::{batch_due_notes, plan_refund, DueNote, RefundPolicy},
    Zatoshi,
};
use serde::Serialize;
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{db, error::ApiError, AppState};

/// Plan refunds for every published auction. Returns the auction codes planned.
pub async fn plan_published(
    state: &AppState,
    policy: &RefundPolicy,
) -> Result<Vec<String>, ApiError> {
    let mut planned = Vec::new();
    for auction in db::auctions_where_status(&state.db, &[AuctionStatus::Published]).await? {
        let mut tx = state.db.begin().await?;
        let bids: Vec<(Uuid, i64)> = sqlx::query_as(
            "SELECT id, refund_due_zat FROM bids \
             WHERE auction_id = $1 AND refund_due_zat >= 10000 AND refund_ua_enc IS NOT NULL",
        )
        .bind(auction.id)
        .fetch_all(&mut *tx)
        .await?;

        let now = Utc::now();
        let mut random = || OsRng.next_u64();
        let mut notes = 0usize;
        for (bid_id, due) in &bids {
            for note in plan_refund(Zatoshi(*due as u64), policy, &mut random) {
                sqlx::query(
                    "INSERT INTO refund_notes (auction_id, bid_id, kind, denomination_zat, not_before) \
                     VALUES ($1, $2, 'refund', $3, $4)",
                )
                .bind(auction.id)
                .bind(bid_id)
                .bind(note.value.0 as i64)
                .bind(now + Duration::from_secs(note.delay_secs))
                .execute(&mut *tx)
                .await?;
                notes += 1;
            }
        }
        let unrefundable: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM bids WHERE auction_id = $1 AND refund_due_zat >= 10000 AND refund_ua_enc IS NULL",
        )
        .bind(auction.id)
        .fetch_one(&mut *tx)
        .await?;

        let moved = sqlx::query(
            "UPDATE auctions SET status = 'refunding', updated_at = now() WHERE id = $1 AND status = 'published'",
        )
        .bind(auction.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if moved == 0 {
            continue; // another worker planned it first; the transaction rolls back
        }
        db::audit(
            &mut *tx,
            Some(auction.id),
            "system",
            None,
            "refunds.planned",
            json!({ "bids": bids.len(), "notes": notes, "needs_manual_review": unrefundable }),
        )
        .await?;
        db::audit(
            &mut *tx,
            Some(auction.id),
            "system",
            None,
            "auction.transition",
            json!({ "from": "published", "to": "refunding" }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(code = %auction.code, notes, "refunds planned");
        planned.push(auction.code);
    }
    Ok(planned)
}

#[derive(Debug, FromRow)]
struct DueRow {
    id: Uuid,
    auction_id: Uuid,
    bid_id: Option<Uuid>,
}

/// Group notes that are due at `now` into refund batches. Returns batches created.
pub async fn batch_due(
    state: &AppState,
    policy: &RefundPolicy,
    now: DateTime<Utc>,
) -> Result<usize, ApiError> {
    let mut tx = state.db.begin().await?;
    let mut due: Vec<DueRow> = sqlx::query_as(
        "SELECT id, auction_id, bid_id FROM refund_notes \
         WHERE status = 'planned' AND kind = 'refund' AND not_before <= $1 FOR UPDATE SKIP LOCKED",
    )
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    // Order must not reveal planning order: shuffle with a CSPRNG before batching.
    due.shuffle(&mut OsRng);

    let mut created = 0;
    let mut auctions: Vec<Uuid> = due.iter().map(|d| d.auction_id).collect();
    auctions.sort();
    auctions.dedup();
    for auction_id in auctions {
        let notes: Vec<DueNote<Uuid>> = due
            .iter()
            .filter(|d| d.auction_id == auction_id)
            .map(|d| DueNote {
                id: d.id,
                owner: d.bid_id.unwrap_or(d.id),
                value: Zatoshi(0),
            })
            .collect();
        for batch in batch_due_notes(&notes, policy.max_outputs_per_batch) {
            let batch_id: Uuid = sqlx::query_scalar(
                "INSERT INTO tx_batches (auction_id, kind, status) VALUES ($1, 'refund', 'building') RETURNING id",
            )
            .bind(auction_id)
            .fetch_one(&mut *tx)
            .await?;
            let ids: Vec<Uuid> = batch.iter().map(|n| n.id).collect();
            sqlx::query(
                "UPDATE refund_notes SET status = 'batched', batch_id = $1 WHERE id = ANY($2)",
            )
            .bind(batch_id)
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
            created += 1;
        }
    }
    tx.commit().await?;
    // TODO(3.9): build each batch's PCZT and open it for trustee signatures.
    Ok(created)
}

/// Background loop: plan new refunds and batch due notes every minute.
pub async fn run(state: AppState, paused: bool) {
    if paused {
        tracing::warn!("refunds paused (SF_REFUNDS_PAUSED)");
        return;
    }
    let policy = RefundPolicy::default();
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tick.tick().await;
        if let Err(err) = plan_published(&state, &policy).await {
            tracing::warn!(error = %err, "refund planning failed");
        }
        if let Err(err) = batch_due(&state, &policy, Utc::now()).await {
            tracing::warn!(error = %err, "refund batching failed");
        }
    }
}

/// Refund progress for an auction (aggregates only).
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RefundProgress {
    pub notes_total: i64,
    pub notes_planned: i64,
    pub notes_batched: i64,
    pub notes_sent: i64,
    pub batches: i64,
    pub needs_manual_review: i64,
}

pub async fn progress(state: &AppState, auction_id: Uuid) -> Result<RefundProgress, ApiError> {
    let row: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE status = 'planned'), count(*) FILTER (WHERE status = 'batched'), \
         count(*) FILTER (WHERE status IN ('broadcast', 'confirmed')) FROM refund_notes WHERE auction_id = $1",
    )
    .bind(auction_id)
    .fetch_one(&state.db)
    .await?;
    let batches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tx_batches WHERE auction_id = $1 AND kind = 'refund'",
    )
    .bind(auction_id)
    .fetch_one(&state.db)
    .await?;
    let manual: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM bids WHERE auction_id = $1 AND refund_due_zat >= 10000 AND refund_ua_enc IS NULL",
    )
    .bind(auction_id)
    .fetch_one(&state.db)
    .await?;
    Ok(RefundProgress {
        notes_total: row.0,
        notes_planned: row.1,
        notes_batched: row.2,
        notes_sent: row.3,
        batches,
        needs_manual_review: manual,
    })
}
