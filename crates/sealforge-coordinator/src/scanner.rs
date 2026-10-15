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

use sealforge_api::types::{AuctionStatus, Network, SpendAuthView};
use sealforge_chain::{
    chain_tip, connect,
    spend::{apply_signatures, pczt_from_bytes, pczt_to_bytes, send_raw, PaymentOut},
    ChainParams, Client, EscrowWallet, ReceivedPayment,
};
use sealforge_core::memo::BidMemo;
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

fn wallet_path(dir: &Path, auction_id: Uuid) -> PathBuf {
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
        let rows = sqlx::query(
            "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, \
             memo_version, commitment, refund_ua_enc, status, invalid_reason) \
             VALUES ($1, $2, $3, 'ironwood', $4, now(), $5, $6, $7, \
                     CASE WHEN $8::text IS NULL THEN NULL ELSE pgp_sym_encrypt($8, $9) END, $10::bid_status, $11) \
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

/// A batch both signers have signed, ready to finalize.
#[derive(sqlx::FromRow)]
struct SignedBatch {
    id: Uuid,
    pczt: Vec<u8>,
    sighash: Vec<u8>,
    spend_auth: serde_json::Value,
    signer_slots: Vec<i16>,
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
        send_raw(client, raw).await?;
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
        let next: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM tx_batches WHERE auction_id = $1 AND status = 'building' ORDER BY created_at LIMIT 1",
        )
        .bind(auction.id)
        .fetch_optional(&state.db)
        .await?;
        if let Some(batch_id) = next {
            let outputs: Vec<(String, i64)> = sqlx::query_as(
                "SELECT CASE WHEN n.kind = 'payout' THEN a.payout_ua ELSE pgp_sym_decrypt(b.refund_ua_enc, $2) END, \
                 n.denomination_zat FROM refund_notes n JOIN auctions a ON a.id = n.auction_id \
                 LEFT JOIN bids b ON b.id = n.bid_id WHERE n.batch_id = $1",
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
            let unsigned = tokio::task::block_in_place(|| wallet.build_payment(&outs))?;
            let spend_auth: Vec<SpendAuthView> = unsigned
                .spends
                .iter()
                .map(|s| SpendAuthView {
                    action_index: s.action_index as u32,
                    alpha: hex::encode(s.alpha),
                })
                .collect();
            let total: i64 = outputs.iter().map(|(_, a)| a).sum();
            sqlx::query(
                "UPDATE tx_batches SET status = 'awaiting_signatures', pczt = $2, sighash = $3, spend_auth = $4, \
                 total_out_zat = $5, output_count = $6 WHERE id = $1 AND status = 'building'",
            )
            .bind(batch_id)
            .bind(pczt_to_bytes(unsigned.pczt)?)
            .bind(&unsigned.sighash[..])
            .bind(serde_json::to_value(&spend_auth)?)
            .bind(total)
            .bind(outputs.len() as i32)
            .execute(&state.db)
            .await?;
            tracing::info!(code = %auction.code, outputs = outputs.len(), "escrow batch ready for trustee signatures");
        }
    }

    // When every refund is confirmed, pay the organiser; when that's confirmed, settle.
    let (open_refunds, payout_notes, open_payout): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE kind = 'refund' AND status <> 'confirmed'), \
                count(*) FILTER (WHERE kind = 'payout'), \
                count(*) FILTER (WHERE kind = 'payout' AND status <> 'confirmed') \
         FROM refund_notes WHERE auction_id = $1",
    )
    .bind(auction.id)
    .fetch_one(&state.db)
    .await?;
    if open_refunds == 0 && payout_notes == 0 {
        let proceeds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(price_paid_zat), 0)::bigint FROM bids WHERE auction_id = $1",
        )
        .bind(auction.id)
        .fetch_one(&state.db)
        .await?;
        if proceeds > 0 {
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
            .bind(proceeds)
            .bind(batch_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            tracing::info!(code = %auction.code, proceeds, "payout batch planned");
        }
    } else if open_refunds == 0 && open_payout == 0 {
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
            &[AuctionStatus::Ready, AuctionStatus::Open],
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
