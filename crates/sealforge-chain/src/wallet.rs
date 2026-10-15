//! A view-only wallet for one auction escrow.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use rand10::{rand_core::UnwrapErr, rngs::SysRng};
use zcash_client_backend::{
    data_api::{
        wallet::{decrypt_and_store_transaction, ConfirmationsPolicy},
        AccountBirthday, AccountPurpose, TransactionDataRequest, TransactionStatus, WalletRead,
        WalletWrite,
    },
    proto::service::{BlockId, TxFilter},
    sync,
    util::SystemClock,
};
use zcash_client_sqlite::{wallet::init::init_wallet_db, WalletDb};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    memo::Memo,
};

use crate::{cache::MemBlockCache, ChainParams, Client};

/// The OS RNG, as the wallet library recommends: nothing to seed or leak.
type Rng = UnwrapErr<SysRng>;
pub(crate) type Db = WalletDb<rusqlite::Connection, ChainParams, SystemClock, Rng>;

/// A payment received by the escrow, as the scanner and auditor see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedPayment {
    pub txid: [u8; 32],
    pub output_index: u32,
    pub block_height: u32,
    pub amount_zat: u64,
    /// Text memo, if the payment carried one.
    pub memo: Option<String>,
}

pub struct EscrowWallet {
    db: Db,
    params: ChainParams,
    path: PathBuf,
}

pub(crate) fn rng() -> Rng {
    UnwrapErr(SysRng)
}

impl EscrowWallet {
    pub(crate) fn open_db(path: &Path, params: ChainParams) -> anyhow::Result<Db> {
        let mut db = WalletDb::for_path(path, params, SystemClock, rng())
            .with_context(|| format!("opening wallet db {}", path.display()))?;
        init_wallet_db(&mut db, None).map_err(|e| anyhow!("initialising wallet db: {e:?}"))?;
        Ok(db)
    }

    /// Create the escrow wallet: import the UFVK as a view-only account whose
    /// birthday is `birthday_height` (no payment can predate the escrow).
    pub async fn create(
        path: &Path,
        params: ChainParams,
        ufvk: &str,
        birthday_height: u32,
        client: &mut Client,
    ) -> anyhow::Result<Self> {
        let mut db = Self::open_db(path, params)?;
        let ufvk =
            UnifiedFullViewingKey::decode(&params, ufvk).map_err(|e| anyhow!("bad UFVK: {e}"))?;
        // The birthday tree state is the state as of the block before the birthday.
        let prior = birthday_height.saturating_sub(1) as u64;
        let tree_state = client
            .get_tree_state(BlockId {
                height: prior,
                hash: vec![],
            })
            .await?
            .into_inner();
        let birthday = AccountBirthday::from_treestate(tree_state, None)
            .map_err(|e| anyhow!("birthday: {e:?}"))?;
        db.import_account_ufvk("escrow", &ufvk, &birthday, AccountPurpose::ViewOnly, None)
            .map_err(|e| anyhow!("importing escrow UFVK: {e:?}"))?;
        Ok(Self {
            db,
            params,
            path: path.to_owned(),
        })
    }

    pub(crate) fn db(&self) -> &Db {
        &self.db
    }

    pub(crate) fn db_mut(&mut self) -> &mut Db {
        &mut self.db
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn params(&self) -> ChainParams {
        self.params
    }

    pub fn open(path: &Path, params: ChainParams) -> anyhow::Result<Self> {
        Ok(Self {
            db: Self::open_db(path, params)?,
            params,
            path: path.to_owned(),
        })
    }

    /// Scan new blocks, then download full transactions to read memos.
    pub async fn sync(&mut self, client: &mut Client) -> anyhow::Result<()> {
        sync_db(&mut self.db, self.params, client).await
    }

    /// Highest block the wallet has fully scanned.
    pub fn scanned_height(&self) -> anyhow::Result<Option<u32>> {
        let summary = self
            .db
            .get_wallet_summary(ConfirmationsPolicy::MIN)
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(summary.map(|s| u32::from(s.fully_scanned_height())))
    }

    /// Every non-change shielded payment the escrow received, oldest first.
    pub fn received(&self) -> anyhow::Result<Vec<ReceivedPayment>> {
        // WalletDb doesn't expose its connection; read through a separate read-only one.
        let conn = rusqlite::Connection::open_with_flags(
            &self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let mut stmt = conn.prepare(
            "SELECT t.txid, n.action_index, t.mined_height, n.value, n.memo
             FROM ironwood_received_notes n JOIN transactions t ON t.id_tx = n.transaction_id
             WHERE n.is_change = 0 AND t.mined_height IS NOT NULL
             UNION ALL
             SELECT t.txid, n.action_index, t.mined_height, n.value, n.memo
             FROM orchard_received_notes n JOIN transactions t ON t.id_tx = n.transaction_id
             WHERE n.is_change = 0 AND t.mined_height IS NOT NULL
             ORDER BY 3, 1, 2",
        )?;
        let rows = stmt.query_map([], |row| {
            let txid: Vec<u8> = row.get(0)?;
            let memo: Option<Vec<u8>> = row.get(4)?;
            Ok((
                txid,
                row.get::<_, u32>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, i64>(3)?,
                memo,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (txid, output_index, block_height, value, memo) = row?;
            let memo = memo.and_then(|bytes: Vec<u8>| match Memo::from_bytes(&bytes) {
                Ok(Memo::Text(text)) => Some(text.to_string()),
                _ => None,
            });
            out.push(ReceivedPayment {
                txid: txid.try_into().map_err(|_| anyhow!("bad txid length"))?,
                output_index,
                block_height,
                amount_zat: value as u64,
                memo,
            });
        }
        Ok(out)
    }
}

/// Scan new blocks into `db`, then fetch full transactions so memos are stored.
pub(crate) async fn sync_db(
    db: &mut Db,
    params: ChainParams,
    client: &mut Client,
) -> anyhow::Result<()> {
    let cache = MemBlockCache::default();
    sync::run(client, &params, &cache, db, 1_000)
        .await
        .map_err(|e| anyhow!("sync failed: {e}"))?;
    enhance(db, params, client).await
}

async fn enhance(db: &mut Db, params: ChainParams, client: &mut Client) -> anyhow::Result<()> {
    // Requests can produce further requests, so loop until none are left (bounded).
    for _ in 0..10 {
        let requests = db
            .transaction_data_requests()
            .map_err(|e| anyhow!("listing data requests: {e:?}"))?;
        if requests.is_empty() {
            return Ok(());
        }
        for request in requests {
            let txid = match &request {
                TransactionDataRequest::GetStatus(txid)
                | TransactionDataRequest::Enhancement(txid) => *txid,
                // Transparent address queries don't apply: escrows are shielded only.
                _ => continue,
            };
            let raw = client
                .get_transaction(TxFilter {
                    block: None,
                    index: 0,
                    hash: txid.as_ref().to_vec(),
                })
                .await;
            let raw = match raw {
                Ok(r) => r.into_inner(),
                Err(status) if status.code() == tonic::Code::NotFound => {
                    db.set_transaction_status(txid, TransactionStatus::TxidNotRecognized)
                        .map_err(|e| anyhow!("{e:?}"))?;
                    continue;
                }
                Err(status) => return Err(status.into()),
            };
            // lightwalletd reports mempool transactions with height 0 (or u64::MAX).
            let mined = (raw.height > 0 && raw.height < u64::from(u32::MAX))
                .then(|| BlockHeight::from_u32(raw.height as u32));
            match request {
                TransactionDataRequest::GetStatus(_) => {
                    let status = match mined {
                        Some(h) => TransactionStatus::Mined(h),
                        None => TransactionStatus::NotInMainChain,
                    };
                    db.set_transaction_status(txid, status)
                        .map_err(|e| anyhow!("{e:?}"))?;
                }
                _ => {
                    let branch_height = match mined {
                        Some(h) => h,
                        None => {
                            db.chain_height()
                                .map_err(|e| anyhow!("{e:?}"))?
                                .ok_or_else(|| anyhow!("wallet has no chain tip"))?
                                + 1
                        }
                    };
                    let tx = Transaction::read(
                        &raw.data[..],
                        BranchId::for_height(&params, branch_height),
                    )
                    .context("parsing transaction")?;
                    decrypt_and_store_transaction(&params, db, &tx, mined)
                        .map_err(|e| anyhow!("storing transaction: {e:?}"))?;
                }
            }
        }
    }
    Ok(())
}
