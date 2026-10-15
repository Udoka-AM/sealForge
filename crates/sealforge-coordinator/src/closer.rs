//! Closing auctions at their close height (task 3.3).
//!
//! An auction closes once the scanner has processed `close_height` plus
//! [`CLOSE_CONFIRMATIONS`] blocks, so no in-range bid can still appear and a
//! shallow reorg can't change the bid set. Closing classifies every payment
//! with `sealforge_core::selection` (the same rules the auditor uses), runs
//! `compute_result`, and stores outcomes for refunds.

use sealforge_api::types::{AuctionStatus, BidMode};
use sealforge_core::{
    clearing::{AuctionMode as CoreMode, AuctionRules},
    memo::{BidMemo, RevealMemo},
    merkle::leaf_hash,
    result::compute_result,
    selection::{
        classify, classify_commit_reveal, included_bids, included_bids_commit_reveal, BidClass,
        BidWindow, Payment, RevealPayment, RevealWindow,
    },
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
    /// Commit-reveal only (decrypted): the refund address as written in the memo.
    memo_refund_ua: Option<String>,
    reveal_price_zat: Option<i64>,
    reveal_salt: Option<Vec<u8>>,
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
            refund_ua: row.memo_refund_ua.clone().unwrap_or_default(),
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

/// A stored reveal payment, as the auditor would decode it from the chain.
fn reveal_payment(auction_code: &str, row: &BidRow) -> Option<RevealPayment> {
    Some(RevealPayment {
        txid: row.txid.as_slice().try_into().ok()?,
        output_index: row.output_index as u32,
        block_height: row.block_height as u32,
        amount: Zatoshi(row.amount_zat as u64),
        memo: RevealMemo {
            // The scanner stores only reveals naming this auction.
            auction_code: auction_code.to_owned(),
            commitment: row.commitment.as_deref()?.try_into().ok()?,
            price_zat: row.reveal_price_zat? as u64,
            salt: row.reveal_salt.as_deref()?.try_into().ok()?,
        },
    })
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
///
/// Commit-reveal auctions close after their reveal window instead: deposits
/// count up to `close_height`, reveals up to `reveal_close_height`.
pub async fn close_auction(state: &AppState, auction: &AuctionRow) -> Result<bool, ApiError> {
    if auction.status != AuctionStatus::Open {
        return Ok(false);
    }
    let close_height = auction.close_height as u32;
    let commit_reveal = auction.bid_mode == BidMode::CommitReveal;
    let scan_to = match (commit_reveal, auction.reveal_close_height) {
        (true, Some(h)) => h as u32,
        (true, None) => {
            return Err(ApiError::Internal(anyhow::anyhow!(
                "commit-reveal auction without a reveal close height"
            )))
        }
        (false, _) => close_height,
    };
    let scanned: Option<i32> =
        sqlx::query_scalar("SELECT last_height FROM scanner_state WHERE auction_id = $1")
            .bind(auction.id)
            .fetch_optional(&state.db)
            .await?;
    match scanned {
        Some(h) if h as u32 >= scan_to + CLOSE_CONFIRMATIONS => {}
        _ => return Ok(false),
    }
    let open_height = auction
        .open_height
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("open auction without open height")))?
        as u32;
    let data_key = match (commit_reveal, state.settings.data_key.clone()) {
        (true, None) => {
            return Err(ApiError::Unavailable(
                "SF_DATA_KEY is not configured".into(),
            ))
        }
        (_, key) => key.unwrap_or_default(),
    };

    let mut tx = state.db.begin().await?;
    let rows: Vec<BidRow> = sqlx::query_as(
        "SELECT id, txid, output_index, block_height, amount_zat, commitment, invalid_reason, \
         CASE WHEN $2 THEN pgp_sym_decrypt(memo_refund_ua_enc, $3) END AS memo_refund_ua, \
         reveal_price_zat, reveal_salt \
         FROM bids WHERE auction_id = $1 AND invalid_reason IS DISTINCT FROM 'fee_buffer' FOR UPDATE",
    )
    .bind(auction.id)
    .bind(commit_reveal)
    .bind(&data_key)
    .fetch_all(&mut *tx)
    .await?;
    let (reveal_rows, rows): (Vec<BidRow>, Vec<BidRow>) = rows
        .into_iter()
        .partition(|r| r.invalid_reason.as_deref() == Some("reveal"));

    let window = BidWindow {
        auction_code: auction.code.clone(),
        open_height,
        close_height,
        reserve: Zatoshi(auction.reserve_zat as u64),
    };
    let payments: Vec<Payment> = rows.iter().map(|r| payment(&auction.code, r)).collect();
    let reveals: Vec<RevealPayment> = reveal_rows
        .iter()
        .filter_map(|r| reveal_payment(&auction.code, r))
        .collect();

    // Per deposit: its class, the amount that is its bid (the payment, or the
    // revealed price), and the amount of the reveal that counted (refunded too).
    struct Judged {
        class: BidClass,
        bid: u64,
        reveal_amount: u64,
        revealed: Option<u64>,
    }
    let rwindow = RevealWindow {
        bid: window.clone(),
        reveal_close_height: scan_to,
    };
    let revealed = commit_reveal.then(|| classify_commit_reveal(&rwindow, &payments, &reveals));
    let (judged, included): (Vec<Judged>, _) = match &revealed {
        Some(out) => (
            out.iter()
                .zip(&payments)
                .map(|(o, p)| Judged {
                    class: o.class,
                    bid: o.price.map_or(p.amount.0, |x| x.0),
                    reveal_amount: o.reveal.map_or(0, |i| reveals[i].amount.0),
                    revealed: o.price.map(|x| x.0),
                })
                .collect(),
            included_bids_commit_reveal(&rwindow, &payments, &reveals),
        ),
        None => (
            classify(&window, &payments)
                .into_iter()
                .zip(&payments)
                .map(|(class, p)| Judged {
                    class,
                    bid: p.amount.0,
                    reveal_amount: 0,
                    revealed: None,
                })
                .collect(),
            included_bids(&window, &payments),
        ),
    };
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

    for ((row, j), payment) in rows.iter().zip(&judged).zip(&payments) {
        let (status, reason) = db_status(j.class);
        // Memo-level problems were judged by the scanner from the raw memo; keep its reason.
        let scanner_reason = row
            .invalid_reason
            .as_deref()
            .filter(|r| matches!(*r, "bad_memo" | "wrong_auction"));
        let reason = scanner_reason.or(reason);
        let deposit = row.amount_zat as u64;
        // Included bids get their leaf and outcome; everything else is refunded in full.
        // In commit-reveal mode the deposit above the price, and the reveal payment,
        // come back too.
        let (leaf, is_winner, price_paid, refund_due) = if j.class.is_included() {
            let commitment = payment
                .memo
                .as_ref()
                .expect("included bids have a memo")
                .commitment;
            let leaf = leaf_hash(&payment.txid, payment.output_index, j.bid, &commitment);
            let outcome = outcomes[&leaf];
            (
                Some(leaf.to_vec()),
                Some(outcome.is_winner),
                outcome.price_paid.0,
                deposit + j.reveal_amount - outcome.price_paid.0,
            )
        } else {
            (None, Some(false), 0, deposit + j.reveal_amount)
        };
        sqlx::query(
            "UPDATE bids SET status = $1::bid_status, invalid_reason = $2, leaf_hash = $3, is_winner = $4, \
             price_paid_zat = $5, refund_due_zat = $6, reveal_price_zat = $7 WHERE id = $8",
        )
        .bind(status)
        .bind(reason)
        .bind(leaf)
        .bind(is_winner)
        .bind(price_paid as i64)
        .bind(refund_due as i64)
        .bind(j.revealed.map(|p| p as i64))
        .bind(row.id)
        .execute(&mut *tx)
        .await?;
    }
    // Reveal payments: one that counted is refunded with its deposit; any other
    // has no refund address and goes to manual review.
    let counted: std::collections::BTreeSet<([u8; 32], u32)> = revealed
        .iter()
        .flatten()
        .filter_map(|o| o.reveal)
        .map(|i| (reveals[i].txid, reveals[i].output_index))
        .collect();
    for row in &reveal_rows {
        let key: ([u8; 32], u32) = (
            row.txid.as_slice().try_into().unwrap_or([0; 32]),
            row.output_index as u32,
        );
        let refund_due = if counted.contains(&key) {
            0
        } else {
            row.amount_zat
        };
        sqlx::query("UPDATE bids SET is_winner = false, price_paid_zat = 0, refund_due_zat = $1 WHERE id = $2")
            .bind(refund_due)
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
    .bind(scan_to as i32)
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
