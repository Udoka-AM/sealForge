//! A spending wallet for local regtest testing (feature `dev-wallet`).
//!
//! Lets tests and demos send real shielded bids, with memos, into an escrow
//! on the local chain (scripts/regtest.sh). Never used with real funds.

use std::{num::NonZeroUsize, path::Path, str::FromStr};

use anyhow::{anyhow, Context};
use secrecy::SecretVec;
use zcash_address::ZcashAddress;
use zcash_client_backend::{
    data_api::{
        wallet::{
            create_proposed_transactions,
            input_selection::{GreedyInputSelector, GreedyInputSelectorError, SpendPolicy},
            propose_transfer, ConfirmationsPolicy, SpendingKeys,
        },
        Account, AccountBirthday, WalletCommitmentTrees, WalletRead, WalletWrite,
    },
    fees::{standard::MultiOutputChangeStrategy, DustOutputPolicy, SplitPolicy, StandardFeeRule},
    proto::service::{BlockId, RawTransaction},
    util::SystemClock,
    wallet::OvkPolicy,
};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_proofs::prover::LocalTxProver;
use zcash_protocol::{
    memo::{Memo, MemoBytes},
    value::Zatoshis,
    ShieldedPool,
};
use zip321::{Payment, TransactionRequest};

use crate::{
    wallet::{rng, sync_db, Db, EscrowWallet},
    ChainParams, Client,
};

pub struct DevWallet {
    db: Db,
    params: ChainParams,
    usk: UnifiedSpendingKey,
}

impl DevWallet {
    /// Create (or reopen) a wallet from a 32-byte test seed, born at `birthday_height`.
    pub async fn open_or_create(
        path: &Path,
        params: ChainParams,
        seed: [u8; 32],
        birthday_height: u32,
        client: &mut Client,
    ) -> anyhow::Result<Self> {
        let existed = path.exists();
        let mut db = EscrowWallet::open_db(path, params)?;
        let seed = SecretVec::new(seed.to_vec());
        let usk = if existed {
            UnifiedSpendingKey::from_seed(&params, &seed_bytes(&seed), zip32::AccountId::ZERO)
                .map_err(|e| anyhow!("{e:?}"))?
        } else {
            let tree_state = client
                .get_tree_state(BlockId {
                    height: birthday_height.saturating_sub(1) as u64,
                    hash: vec![],
                })
                .await?
                .into_inner();
            let birthday = AccountBirthday::from_treestate(tree_state, None)
                .map_err(|e| anyhow!("birthday: {e:?}"))?;
            let (_, usk) = db
                .create_account("dev", &seed, &birthday, None)
                .map_err(|e| anyhow!("creating account: {e:?}"))?;
            usk
        };
        Ok(Self { db, params, usk })
    }

    /// Unified Address with an Orchard receiver (spendable into Ironwood).
    pub fn address(&self) -> anyhow::Result<String> {
        let (ua, _) = self
            .usk
            .to_unified_full_viewing_key()
            .default_address(UnifiedAddressRequest::ORCHARD)
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(ua.encode(&self.params))
    }

    pub async fn sync(&mut self, client: &mut Client) -> anyhow::Result<()> {
        sync_db(&mut self.db, self.params, client).await
    }

    fn account_id(&self) -> anyhow::Result<<Db as WalletRead>::AccountId> {
        let ids = self.db.get_account_ids().map_err(|e| anyhow!("{e:?}"))?;
        ids.into_iter()
            .find(|id| {
                self.db
                    .get_account(*id)
                    .ok()
                    .flatten()
                    .and_then(|a| a.ufvk().cloned())
                    // Match on the Orchard key: the full UFVK also carries a transparent
                    // part that depends on which features the wallet was created with.
                    .is_some_and(|ufvk| {
                        ufvk.orchard().map(|k| k.to_bytes())
                            == self
                                .usk
                                .to_unified_full_viewing_key()
                                .orchard()
                                .map(|k| k.to_bytes())
                    })
            })
            .ok_or_else(|| anyhow!("dev account not found"))
    }

    /// Spendable balance (zatoshi) under the minimum confirmations policy.
    pub fn spendable_zat(&self) -> anyhow::Result<u64> {
        let summary = self
            .db
            .get_wallet_summary(ConfirmationsPolicy::MIN)
            .map_err(|e| anyhow!("{e:?}"))?
            .ok_or_else(|| anyhow!("wallet not synced"))?;
        let id = self.account_id()?;
        Ok(summary
            .account_balances()
            .get(&id)
            .map(|b| u64::from(b.spendable_value()))
            .unwrap_or(0))
    }

    /// Send `amount_zat` to `to` with an optional text memo. Returns the txid (hex, display order).
    pub async fn send(
        &mut self,
        client: &mut Client,
        to: &str,
        amount_zat: u64,
        memo: Option<&str>,
    ) -> anyhow::Result<String> {
        let payment = Payment::new(
            ZcashAddress::from_str(to).map_err(|e| anyhow!("bad recipient: {e}"))?,
            Some(Zatoshis::from_u64(amount_zat).map_err(|_| anyhow!("bad amount"))?),
            memo.map(|m| Memo::from_str(m).map(MemoBytes::from))
                .transpose()
                .map_err(|e| anyhow!("bad memo: {e:?}"))?,
            None,
            None,
            vec![],
        )
        .map_err(|e| anyhow!("payment: {e:?}"))?;
        let request =
            TransactionRequest::new(vec![payment]).map_err(|e| anyhow!("request: {e:?}"))?;

        let change = MultiOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            ShieldedPool::Ironwood,
            DustOutputPolicy::default(),
            SplitPolicy::with_min_output_value(NonZeroUsize::MIN, Zatoshis::const_from_u64(10_000)),
        );
        let account = self.account_id()?;
        let proposal = propose_transfer::<_, _, _, _, <Db as WalletCommitmentTrees>::Error>(
            &mut self.db,
            &self.params,
            account,
            &GreedyInputSelector::new(),
            &change,
            request,
            ConfirmationsPolicy::MIN,
            &SpendPolicy::default(),
            None,
            None,
        )
        .map_err(|e| anyhow!("proposing: {e}"))?;

        let prover = LocalTxProver::bundled();
        let txids = create_proposed_transactions::<
            _,
            _,
            GreedyInputSelectorError,
            _,
            std::convert::Infallible,
            _,
        >(
            &mut self.db,
            &self.params,
            &SystemClock,
            &mut rng(),
            &prover,
            &prover,
            &SpendingKeys::from_unified_spending_key(self.usk.clone()),
            OvkPolicy::Sender,
            &proposal,
            None,
        )
        .map_err(|e| anyhow!("building: {e}"))?;
        let txid = *txids.first();
        let tx = self
            .db
            .get_transaction(txid)
            .map_err(|e| anyhow!("{e:?}"))?
            .context("built transaction not found")?;
        let mut raw = RawTransaction::default();
        tx.write(&mut raw.data)?;
        let response = client.send_transaction(raw).await?.into_inner();
        if response.error_code != 0 {
            return Err(anyhow!(
                "broadcast failed ({}): {}",
                response.error_code,
                response.error_message
            ));
        }
        Ok(txid.to_string())
    }
}

fn seed_bytes(seed: &SecretVec<u8>) -> Vec<u8> {
    use secrecy::ExposeSecret;
    seed.expose_secret().clone()
}
