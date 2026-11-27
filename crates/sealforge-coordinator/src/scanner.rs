//! Escrow scanner (task 2.5).
//!
//! Every ~25 s (one NU7 block): read the chain tip from Zaino, then for each
//! auction whose escrow can receive bids (`ready`, `open`), sync its escrow
//! wallet and record new payments in `bids`. Memo-level problems (`bad_memo`,
//! `wrong_auction`) are judged here from the raw memo. Everything else
//! (before open, late, duplicates, reserve) is decided at close by
//! `sealforge_core::selection`, the same rules the auditor uses.

use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use sealforge_api::types::{AuctionStatus, BidMode, Network, SpendAuthView};
use sealforge_chain::{
    chain_tip, connect,
    spend::{
        apply_signatures, pczt_from_bytes, pczt_to_bytes, send_raw, PaymentOut, TooManyNotes,
        MAX_NOTES_PER_TX,
    },
    ChainParams, Client, EscrowWallet, ReceivedPayment,
};
use sealforge_core::memo::{parse_fee_buffer_memo, BidMemo, RevealMemo};
use sealforge_escrow::frost::keys::PublicKeyPackage;
use serde_json::json;
use uuid::Uuid;

use crate::{
    db::{self, AuctionRow},
    error::ApiError,
    signing::{aggregate, SignerValues},
    AppState,
};

pub fn chain_params(network: &str) -> ChainParams {
    match network {
        "mainnet" => ChainParams::Mainnet,
        "regtest" => ChainParams::regtest(),
        _ => ChainParams::Testnet,
    }
}

pub(crate) fn wallet_path(dir: &Path, auction_id: Uuid) -> PathBuf {
    dir.join(format!("escrow-{auction_id}.sqlite"))
}

/// Record payments the escrow wallet found. Idempotent: existing (txid, index) rows are skipped.
/// Returns how many new payments were recorded.
pub async fn ingest(
    state: &AppState,
    auction: &AuctionRow,
    payments: &[ReceivedPayment],
) -> Result<usize, ApiError> {
    let data_key = state
        .settings
        .data_key
        .clone()
        .ok_or_else(|| ApiError::Unavailable("SF_DATA_KEY is not configured".into()))?;
    let mut inserted = 0;
    for p in payments {
        // The organiser's fee buffer: kept for fees, never refunded, swept with the payout.
        if p.memo.as_deref().and_then(parse_fee_buffer_memo) == Some(auction.code.as_str()) {
            let rows = sqlx::query(
                "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, \
                 status, invalid_reason, refund_due_zat, is_winner) \
                 VALUES ($1, $2, $3, 'ironwood', $4, now(), $5, 'invalid', 'fee_buffer', 0, false) \
                 ON CONFLICT (txid, output_index) DO NOTHING",
            )
            .bind(auction.id)
            .bind(&p.txid[..])
            .bind(p.output_index as i32)
            .bind(p.block_height as i32)
            .bind(p.amount_zat as i64)
            .execute(&state.db)
            .await?
            .rows_affected();
            if rows > 0 {
                sqlx::query(
                    "UPDATE auctions SET fee_buffer_zat = fee_buffer_zat + $2 WHERE id = $1",
                )
                .bind(auction.id)
                .bind(p.amount_zat as i64)
                .execute(&state.db)
                .await?;
            }
            inserted += rows as usize;
            continue;
        }
        // Commit-reveal: a reveal names a deposit's commitment and opens it. The
        // closer decides whether it counts; its amount is refunded with the deposit.
        let after_close = matches!(
            auction.status,
            AuctionStatus::Closed | AuctionStatus::Published | AuctionStatus::Refunding
        );
        let reveal = p
            .memo
            .as_deref()
            .filter(|_| auction.bid_mode == BidMode::CommitReveal)
            .and_then(|m| RevealMemo::decode(m).ok())
            .filter(|r| r.auction_code == auction.code);
        if let Some(r) = reveal {
            let rows = sqlx::query(
                "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, \
                 memo_version, commitment, status, invalid_reason, reveal_price_zat, reveal_salt, refund_due_zat, is_winner) \
                 VALUES ($1, $2, $3, 'ironwood', $4, now(), $5, 1, $6, 'invalid', 'reveal', $7, $8, $9, \
                         CASE WHEN $9::bigint IS NULL THEN NULL ELSE false END) \
                 ON CONFLICT (txid, output_index) DO NOTHING",
            )
            .bind(auction.id)
            .bind(&p.txid[..])
            .bind(p.output_index as i32)
            .bind(p.block_height as i32)
            .bind(p.amount_zat as i64)
            .bind(&r.commitment[..])
            .bind(r.price_zat as i64)
            .bind(&r.salt[..])
            .bind(after_close.then_some(p.amount_zat as i64))
            .execute(&state.db)
            .await?
            .rows_affected();
            inserted += rows as usize;
            continue;
        }
        let parsed = p.memo.as_deref().map(BidMemo::decode);
        let (status, reason, commitment, refund_ua): (
            &str,
            Option<&str>,
            Option<Vec<u8>>,
            Option<String>,
        ) = match parsed {
            Some(Ok(m)) if m.auction_code == auction.code => (
                "valid",
                None,
                Some(m.commitment.to_vec()),
                Some(m.refund_ua),
            ),
            Some(Ok(m)) => (
                "invalid",
                Some("wrong_auction"),
                Some(m.commitment.to_vec()),
                Some(m.refund_ua),
            ),
            _ => ("invalid", Some("bad_memo"), None, None),
        };
        // Kept as written: commit-reveal commitments bind it, usable or not.
        let memo_refund_ua = refund_ua.clone();
        // A refund address that can't be paid privately (wrong network, transparent,
        // no Orchard receiver) is not stored: the bid still counts, but its refund goes
        // to manual review instead of breaking every batch it would join.
        let refund_ua = refund_ua.filter(|ua| {
            let ok = crate::validate::shielded_ua(ua, auction.network).is_ok();
            if !ok {
                tracing::warn!(code = %auction.code, "bid refund address unusable; manual review");
            }
            ok
        });
        // Once the result is computed (at close), anything new is late and refunded in full.
        let (status, refund_due) = match (after_close, status) {
            (true, "valid") => ("late", Some(p.amount_zat as i64)),
            (true, s) => (s, Some(p.amount_zat as i64)),
            (false, s) => (s, None),
        };
        let rows = sqlx::query(
            "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, \
             memo_version, commitment, refund_ua_enc, status, invalid_reason, refund_due_zat, is_winner, \
             memo_refund_ua_enc) \
             VALUES ($1, $2, $3, 'ironwood', $4, now(), $5, $6, $7, \
                     CASE WHEN $8::text IS NULL THEN NULL ELSE pgp_sym_encrypt($8, $9) END, $10::bid_status, $11, \
                     $12, CASE WHEN $12::bigint IS NULL THEN NULL ELSE false END, \
                     CASE WHEN $13::text IS NULL THEN NULL ELSE pgp_sym_encrypt($13, $9) END) \
             ON CONFLICT (txid, output_index) DO NOTHING",
        )
        .bind(auction.id)
        .bind(&p.txid[..])
        .bind(p.output_index as i32)
        .bind(p.block_height as i32)
        .bind(p.amount_zat as i64)
        .bind(commitment.as_ref().map(|_| 1i16))
        .bind(commitment)
        .bind(refund_ua)
        .bind(&data_key)
        .bind(status)
        .bind(reason)
        .bind(refund_due)
        .bind(memo_refund_ua)
        .execute(&state.db)
        .await?
        .rows_affected();
        inserted += rows as usize;
    }
    Ok(inserted)
}

async fn scan_auction(
    state: &AppState,
    client: &mut Client,
    params: ChainParams,
    dir: &Path,
    auction: &AuctionRow,
) -> anyhow::Result<()> {
    let path = wallet_path(dir, auction.id);
    let mut wallet = if path.exists() {
        EscrowWallet::open(&path, params)?
    } else {
        let data_key = state
            .settings
            .data_key
            .clone()
            .ok_or_else(|| anyhow::anyhow!("SF_DATA_KEY missing"))?;
        let (ufvk, birthday): (Option<String>, Option<i32>) = sqlx::query_as(
            "SELECT pgp_sym_decrypt(escrow_ufvk_enc, $2), escrow_birthday_height FROM auctions WHERE id = $1",
        )
        .bind(auction.id)
        .bind(&data_key)
        .fetch_one(&state.db)
        .await?;
        let ufvk =
            ufvk.ok_or_else(|| anyhow::anyhow!("auction {} has no escrow key", auction.code))?;
        // Without a recorded birthday, fall back to the open height or genesis (slow on public networks).
        let birthday = birthday.or(auction.open_height).unwrap_or(1).max(1) as u32;
        std::fs::create_dir_all(dir)?;
        // Escrow wallets hold viewing-key material: owner-only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        EscrowWallet::create(&path, params, &ufvk, birthday, client).await?
    };
    wallet.sync(client).await?;
    let new = ingest(state, auction, &wallet.received()?).await?;
    if let Some(height) = wallet.scanned_height()? {
        sqlx::query(
            "INSERT INTO scanner_state (auction_id, last_height, last_hash) VALUES ($1, $2, '\\x00') \
             ON CONFLICT (auction_id) DO UPDATE SET last_height = EXCLUDED.last_height, updated_at = now()",
        )
        .bind(auction.id)
        .bind(height as i32)
        .execute(&state.db)
        .await?;
    }
    if new > 0 {
        tracing::info!(code = %auction.code, new, "recorded escrow payments");
    }
    if auction.status == AuctionStatus::Refunding {
        process_batches(state, client, auction, &mut wallet).await?;
    }
    Ok(())
}

static MAX_NOTES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Most escrow notes one transaction may spend (issue #41, ZIP 218).
pub fn max_notes_per_tx() -> usize {
    *MAX_NOTES.get().unwrap_or(&MAX_NOTES_PER_TX)
}

/// Called once at startup on test networks: only ever lowers the limit.
pub fn set_max_notes_per_tx(notes: usize) {
    let _ = MAX_NOTES.set(notes.clamp(2, MAX_NOTES_PER_TX));
}

/// Refunds still owed: refund notes not yet confirmed on chain, plus payments
/// owed a refund that have a refund address but no refund notes planned yet (a
/// late payment, until the next late-refund planning run). While any are open
/// the payout must not sweep their funds and the auction must not settle past
/// them (security review F17).
async fn refunds_owed(state: &AppState, auction_id: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM refund_notes n WHERE n.auction_id = $1 \
                 AND n.kind = 'refund' AND n.status <> 'confirmed') \
              + (SELECT count(*) FROM bids b WHERE b.auction_id = $1 AND b.refund_due_zat >= 10000 \
                 AND b.refund_ua_enc IS NOT NULL \
                 AND NOT EXISTS (SELECT 1 FROM refund_notes n WHERE n.bid_id = b.id))",
    )
    .bind(auction_id)
    .fetch_one(&state.db)
    .await?)
}

/// What comes after the refunds: the next payout, settling, or waiting.
#[derive(Debug, PartialEq, Eq)]
enum AfterRefunds {
    PlanPayout,
    Settle,
    Wait,
}

/// Decide [`AfterRefunds`]. `open_refunds` is [`refunds_owed`].
fn after_refunds(
    open_refunds: i64,
    payout_notes: i64,
    open_payout: i64,
    to_pay: bool,
    more_to_pay: bool,
) -> AfterRefunds {
    if open_refunds > 0 {
        AfterRefunds::Wait
    } else if to_pay && (payout_notes == 0 || (open_payout == 0 && more_to_pay)) {
        AfterRefunds::PlanPayout
    } else if open_payout == 0 && !more_to_pay {
        AfterRefunds::Settle
    } else {
        AfterRefunds::Wait
    }
}

/// Smallest fee buffer worth sweeping back on an auction without proceeds
/// (twice the ZIP-317 minimum fee).
const MIN_SWEEP_ZAT: i64 = 20_000;

/// A batch both signers have signed, ready to finalize.
#[derive(sqlx::FromRow)]
struct SignedBatch {
    id: Uuid,
    pczt: Vec<u8>,
    sighash: Vec<u8>,
    spend_auth: serde_json::Value,
    signer_slots: Vec<i16>,
}

/// Whether a transaction with this expiry height can no longer be mined, given
/// a chain scanned to `scanned` with the transaction not in it. A transaction
/// may be mined at its expiry height but not after (ZIP 203); 0 means no expiry.
fn past_expiry(expiry_height: u32, scanned: u32) -> bool {
    expiry_height != 0 && scanned >= expiry_height
}

/// Expire batches whose transaction can no longer be mined: still waiting for
/// signatures, signed but not broadcast, or broadcast but not mined by its
/// expiry height (the escrow wallet has scanned past it without seeing it).
/// Each is marked `expired` and its notes move to a fresh batch of the same
/// kind, built and signed again. The fresh batch has a new id, so trustees'
/// per-batch nonce files never collide, and trustees' refund ledgers stop
/// counting the expired transaction once the chain is past its expiry height.
async fn expire_batches(
    state: &AppState,
    auction: &AuctionRow,
    wallet: &mut EscrowWallet,
    scanned: u32,
) -> anyhow::Result<()> {
    let live: Vec<(Uuid, Vec<u8>, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT id, pczt, txid FROM tx_batches WHERE auction_id = $1 \
         AND status IN ('awaiting_signatures', 'signed', 'broadcast') AND pczt IS NOT NULL",
    )
    .bind(auction.id)
    .fetch_all(&state.db)
    .await?;
    for (id, pczt, txid) in live {
        let expiry_height = *pczt_from_bytes(&pczt)?.global().expiry_height();
        if !past_expiry(expiry_height, scanned) {
            continue;
        }
        if let Some(txid) = txid {
            let txid: [u8; 32] = txid.try_into().map_err(|_| anyhow::anyhow!("bad txid"))?;
            if wallet.mined_height(&txid)?.is_some() {
                continue; // mined: the confirm step records it
            }
        }
        let mut tx = state.db.begin().await?;
        let moved = sqlx::query(
            "UPDATE tx_batches SET status = 'expired' WHERE id = $1 \
             AND status IN ('awaiting_signatures', 'signed', 'broadcast')",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if moved == 0 {
            continue; // moved on concurrently (e.g. a signer just finished)
        }
        let replacement: Uuid = sqlx::query_scalar(
            "INSERT INTO tx_batches (auction_id, kind, status) \
             SELECT auction_id, kind, 'building' FROM tx_batches WHERE id = $1 RETURNING id",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE refund_notes SET batch_id = $2, status = 'batched' WHERE batch_id = $1",
        )
        .bind(id)
        .bind(replacement)
        .execute(&mut *tx)
        .await?;
        db::audit(
            &mut *tx,
            Some(auction.id),
            "system",
            None,
            "batch.expired",
            json!({ "batch": id, "expiry_height": expiry_height, "replaced_by": replacement }),
        )
        .await?;
        tx.commit().await?;
        tracing::warn!(code = %auction.code, batch = %id, expiry_height, "escrow batch expired unmined; rebuilding");
    }
    Ok(())
}

/// Move refund/payout batches along: build → (trustees sign) → broadcast → confirm.
/// One batch is in flight per auction, so two transactions never pick the same notes.
async fn process_batches(
    state: &AppState,
    client: &mut Client,
    auction: &AuctionRow,
    wallet: &mut EscrowWallet,
) -> anyhow::Result<()> {
    let data_key = state
        .settings
        .data_key
        .clone()
        .ok_or_else(|| anyhow::anyhow!("SF_DATA_KEY missing"))?;

    // Confirm broadcast batches that are now mined.
    let broadcast: Vec<(Uuid, Vec<u8>)> = sqlx::query_as(
        "SELECT id, txid FROM tx_batches WHERE auction_id = $1 AND status = 'broadcast' AND txid IS NOT NULL",
    )
    .bind(auction.id)
    .fetch_all(&state.db)
    .await?;
    for (id, txid) in broadcast {
        let txid: [u8; 32] = txid.try_into().map_err(|_| anyhow::anyhow!("bad txid"))?;
        if let Some(height) = wallet.mined_height(&txid)? {
            sqlx::query(
                "UPDATE tx_batches SET status = 'confirmed', confirmed_height = $2 WHERE id = $1",
            )
            .bind(id)
            .bind(height as i32)
            .execute(&state.db)
            .await?;
            sqlx::query("UPDATE refund_notes SET status = 'confirmed' WHERE batch_id = $1")
                .bind(id)
                .execute(&state.db)
                .await?;
            tracing::info!(code = %auction.code, height, "escrow batch confirmed");
        }
    }

    // Replace batches that expired before they were mined (D3).
    if let Some(scanned) = wallet.scanned_height()? {
        expire_batches(state, auction, wallet, scanned).await?;
    }

    // Catch up any batch whose signer set or signatures are complete.
    let waiting: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM tx_batches WHERE auction_id = $1 AND status = 'awaiting_signatures'",
    )
    .bind(auction.id)
    .fetch_all(&state.db)
    .await?;
    for id in waiting {
        let mut tx = state.db.begin().await?;
        crate::routes::batches::lock(&mut tx, id).await?;
        crate::routes::batches::advance(&mut tx, id).await?;
        tx.commit().await?;
    }

    // Finalize batches both signers have signed.
    let signed: Vec<SignedBatch> = sqlx::query_as(
        "SELECT id, pczt, sighash, spend_auth, signer_slots FROM tx_batches WHERE auction_id = $1 AND status = 'signed'",
    )
    .bind(auction.id)
    .fetch_all(&state.db)
    .await?;
    for SignedBatch {
        id,
        pczt,
        sighash,
        spend_auth,
        signer_slots: slots,
    } in signed
    {
        let spends: Vec<SpendAuthView> = serde_json::from_value(spend_auth)?;
        let rows: Vec<(i16, serde_json::Value, serde_json::Value)> = sqlx::query_as(
            "SELECT t.slot, s.commitments_json, s.shares_json FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
             WHERE s.batch_id = $1 AND t.slot = ANY($2)",
        )
        .bind(id)
        .bind(&slots)
        .fetch_all(&state.db)
        .await?;
        let signers: BTreeMap<u16, SignerValues> = rows
            .into_iter()
            .map(|(slot, c, s)| {
                Ok((
                    slot as u16,
                    SignerValues {
                        commitments: serde_json::from_value(c)?,
                        shares: serde_json::from_value(s)?,
                    },
                ))
            })
            .collect::<anyhow::Result<_>>()?;
        let pkp: Vec<u8> = sqlx::query_scalar(
            "SELECT public_key_package FROM trustees WHERE auction_id = $1 AND public_key_package IS NOT NULL LIMIT 1",
        )
        .bind(auction.id)
        .fetch_one(&state.db)
        .await?;
        let sighash: [u8; 32] = sighash
            .try_into()
            .map_err(|_| anyhow::anyhow!("bad sighash"))?;
        let signatures = aggregate(
            sighash,
            &spends,
            &PublicKeyPackage::deserialize(&pkp)?,
            &signers,
        )?;
        let pczt = apply_signatures(pczt_from_bytes(&pczt)?, sighash, &signatures)?;
        let txid = tokio::task::block_in_place(|| wallet.finalize(pczt))?;
        let raw = wallet.raw_transaction(txid)?;
        // A rejected broadcast stays `signed` and is retried next scan (or
        // replaced once it expires); it mustn't stop the rest of the scan.
        if let Err(err) = send_raw(client, raw).await {
            tracing::warn!(code = %auction.code, batch = %id, error = %err, "escrow batch broadcast failed");
            continue;
        }
        let txid_bytes: [u8; 32] = *txid.as_ref();
        sqlx::query("UPDATE tx_batches SET status = 'broadcast', txid = $2, broadcast_at = now() WHERE id = $1")
            .bind(id)
            .bind(&txid_bytes[..])
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE refund_notes SET status = 'broadcast' WHERE batch_id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
        db::audit(
            &state.db,
            Some(auction.id),
            "system",
            None,
            "batch.broadcast",
            json!({ "batch": id }),
        )
        .await?;
        tracing::info!(code = %auction.code, %txid, "escrow batch broadcast");
    }

    // Build the next batch, only when nothing else is in flight.
    let in_flight: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tx_batches WHERE auction_id = $1 AND status IN ('awaiting_signatures', 'signed', 'broadcast')",
    )
    .bind(auction.id)
    .fetch_one(&state.db)
    .await?;
    if in_flight == 0 {
        // A consolidation goes first: it's what lets the waiting refund fit.
        // Refunds go before a payout: a payout planned before a late payment
        // arrived waits for that payment's refund (F17), so building it first
        // would block the refund forever.
        let next: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, kind::text FROM tx_batches WHERE auction_id = $1 AND status = 'building' \
             ORDER BY (kind = 'consolidate') DESC, (kind = 'refund') DESC, created_at LIMIT 1",
        )
        .bind(auction.id)
        .fetch_optional(&state.db)
        .await?;
        if let Some((batch_id, kind)) = next {
            let max_notes = max_notes_per_tx();
            // Consolidations move notes inside the escrow and have no refund notes.
            let mut total_override: Option<i64> = None;
            let (unsigned, outputs) = if kind == "consolidate" {
                let sweep = tokio::task::block_in_place(|| wallet.build_consolidation(max_notes))?;
                total_override = Some(sweep.amount_zat as i64);
                tracing::info!(code = %auction.code, notes = sweep.notes, "consolidating escrow notes");
                (sweep.payment, 1)
            } else if kind == "payout" {
                // A payment that arrived after the payout was planned is still
                // owed its refund: don't sweep it into the payout (F17). Wait
                // until its refund is planned, sent and confirmed.
                if refunds_owed(state, auction.id).await? > 0 {
                    tracing::info!(code = %auction.code, "payout waits: a late payment's refund isn't confirmed yet");
                    return Ok(());
                }
                // The payout sweeps what's left: proceeds plus any fee buffer and
                // refund dust, net of fees. Payments awaiting manual review stay.
                let held: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(sum(refund_due_zat), 0)::bigint FROM bids \
                     WHERE auction_id = $1 AND refund_due_zat >= 10000 AND refund_ua_enc IS NULL",
                )
                .bind(auction.id)
                .fetch_one(&state.db)
                .await?;
                // sealForge's fee, a second output, once per auction (issue #74):
                // not again if an earlier payout already pays it.
                let proceeds: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(sum(price_paid_zat), 0)::bigint FROM bids WHERE auction_id = $1",
                )
                .bind(auction.id)
                .fetch_one(&state.db)
                .await?;
                let fee_due = sealforge_core::fee::protocol_fee_zat(
                    proceeds.max(0) as u64,
                    auction.fee_bps.max(0) as u16,
                );
                let fee_paid: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM tx_batches WHERE auction_id = $1 AND kind = 'payout' \
                     AND protocol_fee_zat > 0 AND status NOT IN ('expired', 'failed') AND id <> $2)",
                )
                .bind(auction.id)
                .bind(batch_id)
                .fetch_one(&state.db)
                .await?;
                let extra = match (&auction.fee_ua, fee_due > 0 && !fee_paid) {
                    (Some(ua), true) => Some(PaymentOut {
                        address: ua.clone(),
                        amount_zat: fee_due,
                        memo: None,
                    }),
                    _ => None,
                };
                let sweep = tokio::task::block_in_place(|| {
                    wallet.build_sweep(&auction.payout_ua, held as u64, max_notes, extra.as_ref())
                })?;
                sqlx::query("UPDATE tx_batches SET protocol_fee_zat = $2 WHERE id = $1")
                    .bind(batch_id)
                    .bind(if sweep.extra_paid { fee_due as i64 } else { 0 })
                    .execute(&state.db)
                    .await?;
                sqlx::query("UPDATE refund_notes SET denomination_zat = $2 WHERE batch_id = $1")
                    .bind(batch_id)
                    .bind(sweep.amount_zat as i64)
                    .execute(&state.db)
                    .await?;
                // Too many notes for one transaction: another payout follows.
                sqlx::query("UPDATE tx_batches SET more_after = $2 WHERE id = $1")
                    .bind(batch_id)
                    .bind(sweep.more_left)
                    .execute(&state.db)
                    .await?;
                (sweep.payment, 1 + usize::from(sweep.extra_paid))
            } else {
                let outputs: Vec<(String, i64)> = sqlx::query_as(
                    "SELECT pgp_sym_decrypt(b.refund_ua_enc, $2), n.denomination_zat FROM refund_notes n \
                     JOIN bids b ON b.id = n.bid_id WHERE n.batch_id = $1",
                )
                .bind(batch_id)
                .bind(&data_key)
                .fetch_all(&state.db)
                .await?;
                let outs: Vec<PaymentOut> = outputs
                    .iter()
                    .map(|(address, amount)| PaymentOut {
                        address: address.clone(),
                        amount_zat: *amount as u64,
                        memo: None,
                    })
                    .collect();
                // Proving is CPU-heavy; keep it off the async scheduler.
                match tokio::task::block_in_place(|| wallet.build_payment(&outs, max_notes)) {
                    Ok(unsigned) => (unsigned, outputs.len()),
                    // It would spend more notes than a block can hold: merge the
                    // oldest notes first, then this refund is built again.
                    Err(e) if e.downcast_ref::<TooManyNotes>().is_some() => {
                        let planned: i64 = sqlx::query_scalar(
                            "SELECT count(*) FROM tx_batches WHERE auction_id = $1 \
                             AND kind = 'consolidate' AND status = 'building'",
                        )
                        .bind(auction.id)
                        .fetch_one(&state.db)
                        .await?;
                        if planned == 0 {
                            sqlx::query(
                                "INSERT INTO tx_batches (auction_id, kind, status) VALUES ($1, 'consolidate', 'building')",
                            )
                            .bind(auction.id)
                            .execute(&state.db)
                            .await?;
                            tracing::info!(code = %auction.code, error = %e, "refund needs a consolidation first");
                        }
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                }
            };
            let total: i64 = match total_override {
                Some(total) => total,
                None => sqlx::query_scalar(
                    "SELECT COALESCE(sum(denomination_zat), 0)::bigint FROM refund_notes WHERE batch_id = $1",
                )
                .bind(batch_id)
                .fetch_one(&state.db)
                .await?,
            };
            let spend_auth: Vec<SpendAuthView> = unsigned
                .spends
                .iter()
                .map(|s| SpendAuthView {
                    action_index: s.action_index as u32,
                    alpha: hex::encode(s.alpha),
                })
                .collect();
            sqlx::query(
                "UPDATE tx_batches SET status = 'awaiting_signatures', pczt = $2, sighash = $3, spend_auth = $4, \
                 total_out_zat = $5, output_count = $6 WHERE id = $1 AND status = 'building'",
            )
            .bind(batch_id)
            .bind(pczt_to_bytes(unsigned.pczt)?)
            .bind(&unsigned.sighash[..])
            .bind(serde_json::to_value(&spend_auth)?)
            .bind(total)
            .bind(outputs as i32)
            .execute(&state.db)
            .await?;
            tracing::info!(code = %auction.code, outputs, "escrow batch ready for trustee signatures");
        }
    }

    // When every refund is confirmed, pay the organiser; when that's confirmed, settle.
    let (payout_notes, open_payout): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE kind = 'payout'), \
                count(*) FILTER (WHERE kind = 'payout' AND status <> 'confirmed') \
         FROM refund_notes WHERE auction_id = $1",
    )
    .bind(auction.id)
    .fetch_one(&state.db)
    .await?;
    let proceeds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(price_paid_zat), 0)::bigint FROM bids WHERE auction_id = $1",
    )
    .bind(auction.id)
    .fetch_one(&state.db)
    .await?;
    // With no winners the organiser still gets their fee buffer back, unless
    // too little is left to pay a transaction fee; then there's nothing to send.
    let to_pay = proceeds > 0 || auction.fee_buffer_zat >= MIN_SWEEP_ZAT;
    // The last payout stopped at the per-transaction note limit (#41).
    let more_to_pay: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT more_after FROM tx_batches WHERE auction_id = $1 AND kind = 'payout' \
         ORDER BY created_at DESC LIMIT 1), false)",
    )
    .bind(auction.id)
    .fetch_one(&state.db)
    .await?;
    let open_refunds = refunds_owed(state, auction.id).await?;
    let next = after_refunds(open_refunds, payout_notes, open_payout, to_pay, more_to_pay);
    if next == AfterRefunds::PlanPayout {
        let mut tx = state.db.begin().await?;
        let batch_id: Uuid = sqlx::query_scalar(
            "INSERT INTO tx_batches (auction_id, kind, status) VALUES ($1, 'payout', 'building') RETURNING id",
        )
        .bind(auction.id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO refund_notes (auction_id, kind, denomination_zat, not_before, batch_id, status) \
             VALUES ($1, 'payout', $2, now(), $3, 'batched')",
        )
        .bind(auction.id)
        // Planned amount; building the sweep records what's actually left.
        .bind(proceeds + auction.fee_buffer_zat)
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(code = %auction.code, proceeds, "payout batch planned");
    } else if next == AfterRefunds::Settle {
        let moved = sqlx::query(
            "UPDATE auctions SET status = 'settled', settled_at = now(), updated_at = now() WHERE id = $1 AND status = 'refunding'",
        )
        .bind(auction.id)
        .execute(&state.db)
        .await?
        .rows_affected();
        if moved > 0 {
            db::audit(
                &state.db,
                Some(auction.id),
                "system",
                None,
                "auction.transition",
                json!({ "from": "refunding", "to": "settled" }),
            )
            .await?;
            tracing::info!(code = %auction.code, "auction settled");
        }
    }
    Ok(())
}

/// Background loop. Does nothing useful until Zaino is reachable, then keeps
/// the chain tip and every live escrow up to date.
pub async fn run(state: AppState, zaino_url: String, network: Network, wallet_dir: PathBuf) {
    let params = chain_params(match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
        Network::Regtest => "regtest",
    });
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(25));
    let mut client: Option<Client> = None;
    loop {
        tick.tick().await;
        if client.is_none() {
            match connect(&zaino_url).await {
                Ok(c) => client = Some(c),
                Err(err) => {
                    tracing::warn!(error = %err, url = %zaino_url, "scanner can't reach the lightwallet server");
                    continue;
                }
            }
        }
        let c = client.as_mut().expect("connected");
        match chain_tip(c).await {
            Ok(tip) => state.set_chain_tip(tip),
            Err(err) => {
                tracing::warn!(error = %err, "lost the lightwallet server; reconnecting");
                client = None;
                continue;
            }
        }
        let auctions = match db::auctions_where_status(
            &state.db,
            // Keep syncing after close: refunds and the payout are spent from the escrow.
            &[
                AuctionStatus::Ready,
                AuctionStatus::Open,
                AuctionStatus::Closed,
                AuctionStatus::Published,
                AuctionStatus::Refunding,
            ],
        )
        .await
        {
            Ok(a) => a,
            Err(err) => {
                tracing::warn!(error = %err, "scanner couldn't list auctions");
                continue;
            }
        };
        for auction in auctions.iter().filter(|a| a.network == network) {
            if let Err(err) = scan_auction(&state, c, params, &wallet_dir, auction).await {
                tracing::warn!(code = %auction.code, error = %err, "escrow scan failed");
            }
        }
    }
}

#[cfg(test)]
mod expiry_tests {
    use super::past_expiry;

    #[test]
    fn a_transaction_expires_once_the_chain_reaches_its_expiry_height_without_it() {
        // Mineable at its expiry height, so not expired one block before.
        assert!(!past_expiry(100, 99));
        assert!(past_expiry(100, 100));
        assert!(past_expiry(100, 140));
        // 0: no expiry.
        assert!(!past_expiry(0, 1_000_000));
    }
}

#[cfg(test)]
mod after_refunds_tests {
    use super::{after_refunds, AfterRefunds};

    #[test]
    fn a_late_payment_holds_the_payout_and_settlement() {
        // Refunds all confirmed, but a late payment's refund isn't planned yet
        // (counted in open_refunds): neither pay out nor settle (F17).
        assert_eq!(after_refunds(1, 0, 0, true, false), AfterRefunds::Wait);
        assert_eq!(after_refunds(1, 1, 0, true, false), AfterRefunds::Wait);
    }

    #[test]
    fn otherwise_pay_out_then_settle() {
        assert_eq!(
            after_refunds(0, 0, 0, true, false),
            AfterRefunds::PlanPayout
        );
        assert_eq!(after_refunds(0, 1, 1, true, false), AfterRefunds::Wait);
        assert_eq!(after_refunds(0, 1, 0, true, true), AfterRefunds::PlanPayout);
        assert_eq!(after_refunds(0, 1, 0, true, false), AfterRefunds::Settle);
        // Nothing to pay (no proceeds, too small a fee buffer): settle.
        assert_eq!(after_refunds(0, 0, 0, false, false), AfterRefunds::Settle);
    }
}
