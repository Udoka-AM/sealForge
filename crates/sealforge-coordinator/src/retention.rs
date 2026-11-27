//! Data retention (#65, docs/05-BACKEND-SCHEMA.md §5.8).
//!
//! Thirty days after an auction is settled (or cancelled), the coordinator
//! erases the secrets it no longer needs:
//!
//! - every bidder's encrypted refund address (`bids.refund_ua_enc`,
//!   `bids.memo_refund_ua_enc`): all refunds confirmed long ago;
//! - the escrow's encrypted full viewing key (`auctions.escrow_ufvk_enc`) and
//!   the escrow wallet file holding it: a settled escrow is no longer scanned
//!   or spent.
//!
//! What stays is public evidence: the published result, the bids (without
//! refund addresses), the batches and the audit log, which records each
//! erasure with counts, never values. Anything paid to a settled escrow after
//! that can still be swept by two trustees from their backups
//! (`sealforge escrow recover-start`). Key-generation packages are already
//! deleted when key generation completes (`routes/dkg.rs`).

use std::{path::Path, time::Duration};

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;

/// Days after settlement or cancellation before secrets are erased.
pub const RETENTION_DAYS: i32 = 30;

/// What one pass erased.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    /// Auctions whose secrets were erased.
    pub auctions: u64,
    /// Refund addresses erased across those auctions.
    pub refund_addresses: u64,
    /// Escrow wallet files removed.
    pub wallet_files: u64,
}

/// One retention pass. Idempotent: an auction is erased once
/// (`data_erased_at`), and a wallet file left behind by a failed removal is
/// retried on the next pass.
pub async fn run_once(db: &PgPool, wallet_dir: &Path, days: i32) -> anyhow::Result<Pass> {
    let mut pass = Pass::default();

    let due: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, code FROM auctions WHERE data_erased_at IS NULL \
         AND ((status = 'settled' AND settled_at < now() - make_interval(days => $1)) \
           OR (status = 'cancelled' AND updated_at < now() - make_interval(days => $1)))",
    )
    .bind(days)
    .fetch_all(db)
    .await?;

    for (id, code) in due {
        let mut tx = db.begin().await?;
        let refund_addresses = sqlx::query(
            "UPDATE bids SET refund_ua_enc = NULL, memo_refund_ua_enc = NULL \
             WHERE auction_id = $1 AND (refund_ua_enc IS NOT NULL OR memo_refund_ua_enc IS NOT NULL)",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let had_key: bool = sqlx::query_scalar(
            "SELECT escrow_ufvk_enc IS NOT NULL FROM auctions WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE auctions SET escrow_ufvk_enc = NULL, data_erased_at = now(), updated_at = now() \
             WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        db::audit(
            &mut *tx,
            Some(id),
            "system",
            None,
            "retention.erased",
            json!({
                "refund_addresses": refund_addresses,
                "escrow_viewing_key": had_key,
                "after_days": days,
            }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(%code, refund_addresses, "erased a finished auction's secrets");
        pass.auctions += 1;
        pass.refund_addresses += refund_addresses;
    }

    // Wallet files of erased auctions, including any a previous pass couldn't
    // remove.
    let erased: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM auctions WHERE data_erased_at IS NOT NULL")
            .fetch_all(db)
            .await?;
    for id in erased {
        let path = crate::scanner::wallet_path(wallet_dir, id);
        match std::fs::remove_file(&path) {
            Ok(()) => pass.wallet_files += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = %e, "couldn't remove an erased escrow's wallet file"),
        }
    }

    Ok(pass)
}

/// Run a retention pass every hour.
pub async fn run(db: PgPool, wallet_dir: std::path::PathBuf) {
    let mut tick = tokio::time::interval(Duration::from_secs(3600));
    loop {
        tick.tick().await;
        match run_once(&db, &wallet_dir, RETENTION_DAYS).await {
            Ok(pass) if pass != Pass::default() => tracing::info!(
                auctions = pass.auctions,
                refund_addresses = pass.refund_addresses,
                wallet_files = pass.wallet_files,
                "retention pass"
            ),
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "retention pass failed"),
        }
    }
}
