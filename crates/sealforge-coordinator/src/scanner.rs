//! Escrow scanner (task 2.5).
//!
//! Every ~25 s (one NU7 block): read the chain tip from Zaino, then for each
//! auction whose escrow can receive bids (`ready`, `open`), sync its escrow
//! wallet and record new payments in `bids`. Memo-level problems (`bad_memo`,
//! `wrong_auction`) are judged here from the raw memo. Everything else
//! (before open, late, duplicates, reserve) is decided at close by
//! `sealforge_core::selection`, the same rules the auditor uses.

use std::path::{Path, PathBuf};

use sealforge_api::types::{AuctionStatus, Network};
use sealforge_chain::{chain_tip, connect, ChainParams, Client, EscrowWallet, ReceivedPayment};
use sealforge_core::memo::BidMemo;
use uuid::Uuid;

use crate::{
    db::{self, AuctionRow},
    error::ApiError,
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
