//! Payments from the wallet: Ironwood notes in, Ironwood outputs out.
//!
//! Built with `zcash_primitives`' builder, then the same PCZT roles the escrow
//! uses (`sealforge-chain::spend`): IO finalizer, Ironwood prover, signer,
//! transaction extractor (which checks the proofs and signatures).

use std::{str::FromStr, sync::OnceLock};

use orchard::{
    circuit::{OrchardCircuitVersion, ProvingKey, VerifyingKey},
    keys::Scope,
};
use pczt::roles::{
    creator::Creator, io_finalizer::IoFinalizer, prover::Prover, signer::Signer,
    tx_extractor::TransactionExtractor,
};
use rand10::{rand_core::UnwrapErr, rngs::SysRng};
use serde::{Deserialize, Serialize};
use zcash_keys::address::Address;
use zcash_primitives::transaction::{
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317::FeeRule,
};
use zcash_protocol::{
    consensus::BlockHeight,
    memo::{Memo, MemoBytes},
    value::Zatoshis,
};

use crate::{Pool, SentTx, Wallet, EXPIRY_DELTA};

/// The circuit for current Orchard-shaped proofs (as `sealforge-chain::spend`).
fn circuit() -> OrchardCircuitVersion {
    OrchardCircuitVersion::PostNu6_3
}

/// Building the proving key takes a few seconds; do it once per page.
fn proving_key() -> &'static ProvingKey {
    static PK: OnceLock<ProvingKey> = OnceLock::new();
    PK.get_or_init(|| ProvingKey::build(circuit()))
}

fn verifying_key() -> &'static VerifyingKey {
    static VK: OnceLock<VerifyingKey> = OnceLock::new();
    VK.get_or_init(|| VerifyingKey::build(circuit()))
}

/// Warm the proving key up before the user presses "Pay".
pub fn prepare_prover() {
    let _ = proving_key();
    let _ = verifying_key();
}

#[derive(Clone, Debug, Deserialize)]
pub struct PaymentRequest {
    /// A Unified Address with an Orchard receiver.
    pub to: String,
    pub amount_zat: u64,
    pub memo: Option<String>,
    pub label: Option<String>,
}

/// A signed transaction, ready to broadcast.
#[derive(Debug, Serialize)]
pub struct Built {
    pub txid: String,
    #[serde(with = "hex_bytes")]
    pub raw: Vec<u8>,
    pub fee_zat: u64,
    pub expiry_height: u32,
}

mod hex_bytes {
    pub fn serialize<S: serde::Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }
}

type BuilderOf = Builder<sealforge_scan::ChainParams, ()>;

/// The OS RNG, as `sealforge-chain` uses it.
fn rng() -> UnwrapErr<SysRng> {
    UnwrapErr(SysRng)
}

impl Wallet {
    fn builder(&self, target: BlockHeight) -> BuilderOf {
        Builder::new(
            self.keys.params(),
            target,
            BuildConfig::Standard {
                sapling_anchor: None,
                orchard_anchor: None,
                ironwood_anchor: Some(orchard::Anchor::from(self.ironwood.tree.root())),
                orchard_padding: BundlePadding::DEFAULT,
                ironwood_padding: BundlePadding::DEFAULT,
            },
        )
    }

    /// The fee for spending `spends` notes into `outputs` Ironwood outputs (ZIP 317),
    /// asked of the builder itself so it matches what `build_for_pczt` checks.
    fn fee_for(
        &self,
        target: BlockHeight,
        spends: &[usize],
        outputs: usize,
    ) -> Result<u64, String> {
        let mut b = self.builder(target);
        let fvk = self.keys.fvk.clone();
        for &i in spends {
            let (record, witness) = &self.ironwood.notes[i];
            let path = witness.path().ok_or("note has no witness")?;
            b.add_ironwood_spend::<()>(fvk.clone(), record.note()?, path.into())
                .map_err(|e| format!("{e:?}"))?;
        }
        let addr = self.keys.change_address();
        for _ in 0..outputs {
            b.add_ironwood_output::<()>(None, addr, Zatoshis::ZERO, MemoBytes::empty())
                .map_err(|e| format!("{e:?}"))?;
        }
        b.get_fee(&FeeRule::standard())
            .map(u64::from)
            .map_err(|e| format!("fee: {e:?}"))
    }

    /// Spendable Ironwood notes, largest first (indices into the pool).
    fn spendable_indices(&self) -> Vec<usize> {
        let pending = self.pending_nullifiers();
        let mut idx: Vec<usize> = self
            .ironwood
            .notes
            .iter()
            .enumerate()
            .filter(|(_, (n, _))| {
                n.pool == Pool::Ironwood
                    && n.spent_txid.is_none()
                    && n.note_version == 3
                    && !pending.contains(&n.nullifier.as_str())
            })
            .map(|(i, _)| i)
            .collect();
        idx.sort_by_key(|&i| std::cmp::Reverse(self.ironwood.notes[i].0.value_zat));
        idx
    }

    /// The fee a payment of `amount_zat` would pay, or an error if the
    /// spendable balance doesn't cover it.
    pub fn quote(&self, amount_zat: u64) -> Result<u64, String> {
        self.select(amount_zat).map(|(_, fee)| fee)
    }

    fn select(&self, amount_zat: u64) -> Result<(Vec<usize>, u64), String> {
        let target = BlockHeight::from_u32(self.scanned_height + 1);
        let mut chosen = Vec::new();
        let mut total = 0u64;
        for i in self.spendable_indices() {
            chosen.push(i);
            total += self.ironwood.notes[i].0.value_zat;
            // One payment output plus change.
            let fee = self.fee_for(target, &chosen, 2)?;
            if total >= amount_zat + fee {
                return Ok((chosen, fee));
            }
        }
        let fee = self.fee_for(target, &chosen, 2).unwrap_or(10_000);
        Err(format!(
            "Not enough spendable funds: {} zat available, {} zat needed including the {} zat fee.",
            total,
            amount_zat + fee,
            fee
        ))
    }

    /// Build, prove and sign a payment. The notes it spends are locked until
    /// it confirms or expires; nothing is marked paid until its spends are seen
    /// on chain ([`Wallet::scan_block`]).
    pub fn pay(&mut self, request: &PaymentRequest) -> Result<Built, String> {
        let ask = self.keys.ask.clone().ok_or("Unlock the wallet to pay.")?;
        let params = self.keys.params();
        let recipient = match Address::decode(&params, request.to.trim()) {
            Some(Address::Unified(ua)) => ua
                .orchard()
                .copied()
                .ok_or("That address has no shielded Orchard receiver.")?,
            Some(_) => return Err("Pay to a Unified Address with an Orchard receiver.".into()),
            None => return Err(format!("That isn't a {} Unified Address.", self.network)),
        };
        if request.amount_zat == 0 {
            return Err("Enter an amount above zero.".into());
        }
        let memo = match &request.memo {
            Some(text) => {
                MemoBytes::from(Memo::from_str(text).map_err(|e| format!("memo: {e:?}"))?)
            }
            None => MemoBytes::empty(),
        };

        let (chosen, fee) = self.select(request.amount_zat)?;
        let total: u64 = chosen
            .iter()
            .map(|&i| self.ironwood.notes[i].0.value_zat)
            .sum();
        let change = total - request.amount_zat - fee;
        let target = BlockHeight::from_u32(self.scanned_height + 1);

        let fvk = self.keys.fvk.clone();
        let mut builder = self.builder(target);
        let mut spends = Vec::new();
        for &i in &chosen {
            let (record, witness) = &self.ironwood.notes[i];
            let path = witness.path().ok_or("note has no witness")?;
            builder
                .add_ironwood_spend::<()>(fvk.clone(), record.note()?, path.into())
                .map_err(|e| format!("adding spend: {e:?}"))?;
            spends.push(record.nullifier.clone());
        }
        let amount = Zatoshis::from_u64(request.amount_zat).map_err(|_| "bad amount")?;
        builder
            .add_ironwood_output::<()>(Some(fvk.to_ovk(Scope::External)), recipient, amount, memo)
            .map_err(|e| format!("adding output: {e:?}"))?;
        builder
            .add_ironwood_output::<()>(
                Some(fvk.to_ovk(Scope::Internal)),
                self.keys.change_address(),
                Zatoshis::from_u64(change).map_err(|_| "bad change")?,
                MemoBytes::empty(),
            )
            .map_err(|e| format!("adding change: {e:?}"))?;

        let result = builder
            .build_for_pczt(rng(), &FeeRule::standard())
            .map_err(|e| format!("building: {e:?}"))?;
        let spend_indices: Vec<usize> = (0..chosen.len())
            .map(|i| {
                result
                    .ironwood_meta
                    .spend_action_index(i)
                    .ok_or_else(|| format!("spend {i} missing from the bundle"))
            })
            .collect::<Result<_, _>>()?;
        let pczt = Creator::build_from_parts(result.pczt_parts).ok_or("PCZT creation failed")?;
        let pczt = IoFinalizer::new(pczt)
            .finalize_io(rng())
            .map_err(|e| format!("IO finalizer: {e:?}"))?;
        let pczt = Prover::new(pczt)
            .create_ironwood_proof(rng(), proving_key())
            .map_err(|e| format!("proving: {e:?}"))?
            .finish();
        let mut signer = Signer::new(pczt).map_err(|e| format!("signer: {e:?}"))?;
        for index in spend_indices {
            signer
                .sign_ironwood(rng(), index, &ask)
                .map_err(|e| format!("signing: {e:?}"))?;
        }
        let tx = TransactionExtractor::new(signer.finish())
            .with_orchard(verifying_key())
            .extract(rng())
            .map_err(|e| format!("extracting: {e:?}"))?;
        let mut raw = Vec::new();
        tx.write(&mut raw).map_err(|e| e.to_string())?;
        let txid = tx.txid().to_string();
        let expiry_height = self.scanned_height + 1 + EXPIRY_DELTA;
        self.sent.push(SentTx {
            txid: txid.clone(),
            to: request.to.trim().to_owned(),
            amount_zat: request.amount_zat,
            fee_zat: fee,
            memo: request.memo.clone(),
            label: request.label.clone(),
            built_at_height: self.scanned_height,
            expiry_height,
            spends,
            mined_height: None,
            expired: false,
        });
        Ok(Built {
            txid,
            raw,
            fee_zat: fee,
            expiry_height,
        })
    }

    /// Forget a payment the lightwallet server refused, so its notes are spendable again.
    pub fn forget_payment(&mut self, txid: &str) {
        self.sent
            .retain(|t| t.txid != txid || t.mined_height.is_some());
    }
}
