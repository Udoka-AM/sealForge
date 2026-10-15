//! Spending from a FROST escrow (tasks 1.5 and 3.9).
//!
//! 1. [`EscrowWallet::build_payment`] proposes the payments, creates a PCZT for
//!    the view-only escrow account, finalizes its inputs and outputs, and adds
//!    the Ironwood proof. It returns the shielded sighash and, for every escrow
//!    spend, the action index and its randomizer α.
//! 2. Trustees produce a 2-of-3 FROST signature per spend (`sealforge-escrow::signing`).
//! 3. [`apply_signatures`] checks each signature against the action's `rk` and
//!    applies it.
//! 4. [`EscrowWallet::finalize`] extracts the transaction, stores it in the
//!    wallet, and [`EscrowWallet::broadcast`] sends it.

use std::{num::NonZeroUsize, str::FromStr};

use anyhow::{anyhow, Context};
use orchard::{
    circuit::{OrchardCircuitVersion, ProvingKey, VerifyingKey},
    primitives::redpallas,
};
use pczt::{
    roles::{io_finalizer::IoFinalizer, low_level_signer, prover::Prover, signer::Signer},
    Pczt,
};
use zcash_address::ZcashAddress;
use zcash_client_backend::{
    data_api::{
        error::Error as DataApiError,
        wallet::{
            create_pczt_from_proposal, extract_and_store_transaction_from_pczt,
            input_selection::{GreedyInputSelector, GreedyInputSelectorError, SpendPolicy},
            propose_transfer, ConfirmationsPolicy,
        },
        InputSource, WalletCommitmentTrees, WalletRead,
    },
    fees::{standard::MultiOutputChangeStrategy, DustOutputPolicy, SplitPolicy, StandardFeeRule},
    proposal::Proposal,
    proto::service::RawTransaction,
    util::SystemClock,
    wallet::OvkPolicy,
};
use zcash_primitives::transaction::builder::BundlePadding;
use zcash_protocol::{
    memo::{Memo, MemoBytes},
    value::Zatoshis,
    ShieldedPool, TxId,
};
use zip321::{Payment, TransactionRequest};

use crate::{
    wallet::{rng, Db, EscrowWallet},
    Client,
};

/// One output of an escrow payment (a refund note or the organiser payout).
#[derive(Clone, Debug)]
pub struct PaymentOut {
    pub address: String,
    pub amount_zat: u64,
    pub memo: Option<String>,
}

/// What trustees sign for one escrow spend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendAuthItem {
    /// Index of the Ironwood action this signature is for.
    pub action_index: usize,
    /// The action's spend authorization randomizer α (little-endian Pallas scalar).
    pub alpha: [u8; 32],
}

/// A proved, unsigned escrow transaction.
pub struct UnsignedPayment {
    pub pczt: Pczt,
    /// Shielded sighash every spend authorization signs.
    pub sighash: [u8; 32],
    pub spends: Vec<SpendAuthItem>,
}

/// `pczt::roles::low_level_signer::OrchardParseError` isn't a `std::error::Error`, so carry it beside other errors.
#[derive(Debug)]
enum SignError {
    Parse(pczt::roles::low_level_signer::OrchardParseError),
    Other(anyhow::Error),
}

impl From<pczt::roles::low_level_signer::OrchardParseError> for SignError {
    fn from(e: pczt::roles::low_level_signer::OrchardParseError) -> Self {
        Self::Parse(e)
    }
}

impl From<SignError> for anyhow::Error {
    fn from(e: SignError) -> Self {
        match e {
            SignError::Parse(p) => anyhow!("PCZT Ironwood bundle: {p:?}"),
            SignError::Other(o) => o,
        }
    }
}

type EscrowProposal = Proposal<StandardFeeRule, <Db as InputSource>::NoteRef>;

enum ProposeFail {
    Insufficient { available: u64, required: u64 },
    Other(anyhow::Error),
}

impl From<anyhow::Error> for ProposeFail {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

impl From<ProposeFail> for anyhow::Error {
    fn from(e: ProposeFail) -> Self {
        match e {
            ProposeFail::Insufficient {
                available,
                required,
            } => anyhow!("proposing escrow payment: insufficient balance (have {available}, need {required} including fee)"),
            ProposeFail::Other(o) => o,
        }
    }
}

fn circuit() -> OrchardCircuitVersion {
    OrchardCircuitVersion::PostNu6_3
}

impl EscrowWallet {
    fn account(&self) -> anyhow::Result<<Db as WalletRead>::AccountId> {
        self.db()
            .get_account_ids()
            .map_err(|e| anyhow!("{e:?}"))?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("escrow wallet has no account"))
    }

    fn propose(&mut self, outputs: &[PaymentOut]) -> Result<EscrowProposal, ProposeFail> {
        let payments = outputs
            .iter()
            .map(|o| {
                Payment::new(
                    ZcashAddress::from_str(&o.address).map_err(|e| anyhow!("bad address: {e}"))?,
                    Some(Zatoshis::from_u64(o.amount_zat).map_err(|_| anyhow!("bad amount"))?),
                    o.memo
                        .as_deref()
                        .map(|m| Memo::from_str(m).map(MemoBytes::from))
                        .transpose()
                        .map_err(|e| anyhow!("bad memo: {e:?}"))?,
                    None,
                    None,
                    vec![],
                )
                .map_err(|e| anyhow!("payment: {e:?}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let request = TransactionRequest::new(payments).map_err(|e| anyhow!("request: {e:?}"))?;
        let change = MultiOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            ShieldedPool::Ironwood,
            DustOutputPolicy::default(),
            SplitPolicy::with_min_output_value(NonZeroUsize::MIN, Zatoshis::const_from_u64(10_000)),
        );
        let account = self.account()?;
        let params = self.params();
        propose_transfer::<_, _, _, _, <Db as WalletCommitmentTrees>::Error>(
            self.db_mut(),
            &params,
            account,
            &GreedyInputSelector::new(),
            &change,
            request,
            ConfirmationsPolicy::MIN,
            &SpendPolicy::default(),
            None,
            None,
        )
        .map_err(|e| match e {
            DataApiError::InsufficientFunds {
                available,
                required,
            } => ProposeFail::Insufficient {
                available: available.into(),
                required: required.into(),
            },
            other => ProposeFail::Other(anyhow!("proposing escrow payment: {other}")),
        })
    }

    /// Propose and prove a payment from the escrow. Proving takes a while
    /// (building the Ironwood proving key plus the proof itself).
    pub fn build_payment(&mut self, outputs: &[PaymentOut]) -> anyhow::Result<UnsignedPayment> {
        let proposal = self.propose(outputs).map_err(anyhow::Error::from)?;
        self.prove(&proposal)
    }

    /// Pay everything in the escrow to `address` except `keep_zat`, net of the
    /// ZIP-317 fee (the organiser payout). Returns the payment and the amount sent.
    pub fn build_sweep(
        &mut self,
        address: &str,
        keep_zat: u64,
    ) -> anyhow::Result<(UnsignedPayment, u64)> {
        let available = self.spendable_zat()?;
        // ZIP-317 minimum: two logical actions.
        let mut fee = 10_000u64;
        for _ in 0..5 {
            let amount = available
                .checked_sub(keep_zat + fee)
                .filter(|a| *a > 0)
                .ok_or_else(|| anyhow!("escrow holds {available} zat: nothing left to sweep"))?;
            let out = PaymentOut {
                address: address.to_owned(),
                amount_zat: amount,
                memo: None,
            };
            match self.propose(std::slice::from_ref(&out)) {
                Ok(proposal) => return Ok((self.prove(&proposal)?, amount)),
                // The fee depends on the notes selected; learn it from the shortfall.
                Err(ProposeFail::Insufficient { required, .. }) => {
                    fee = required.saturating_sub(amount).max(fee + 5_000);
                }
                Err(ProposeFail::Other(e)) => return Err(e),
            }
        }
        Err(anyhow!("couldn't size the sweep"))
    }

    /// Spendable escrow balance under the minimum confirmations policy.
    pub fn spendable_zat(&self) -> anyhow::Result<u64> {
        let account = self.account()?;
        let summary = self
            .db()
            .get_wallet_summary(ConfirmationsPolicy::MIN)
            .map_err(|e| anyhow!("{e:?}"))?
            .ok_or_else(|| anyhow!("wallet not synced"))?;
        Ok(summary
            .account_balances()
            .get(&account)
            .map(|b| u64::from(b.spendable_value()))
            .unwrap_or(0))
    }

    fn prove(&mut self, proposal: &EscrowProposal) -> anyhow::Result<UnsignedPayment> {
        let account = self.account()?;
        let params = self.params();
        let pczt = create_pczt_from_proposal::<
            _,
            _,
            GreedyInputSelectorError,
            _,
            std::convert::Infallible,
            _,
        >(
            self.db_mut(),
            &params,
            &SystemClock,
            &mut rng(),
            account,
            OvkPolicy::Sender,
            proposal,
            None,
            BundlePadding::DEFAULT,
        )
        .map_err(|e| anyhow!("creating PCZT: {e}"))?;

        let pczt = IoFinalizer::new(pczt)
            .finalize_io(rng())
            .map_err(|e| anyhow!("IO finalizer: {e:?}"))?;
        let pczt = Prover::new(pczt)
            .create_ironwood_proof(rng(), &ProvingKey::build(circuit()))
            .map_err(|e| anyhow!("proving: {e:?}"))?
            .finish();

        let signer = Signer::new(pczt).map_err(|e| anyhow!("signer: {e:?}"))?;
        let sighash = signer.shielded_sighash();
        let pczt = signer.finish();

        let mut spends = Vec::new();
        let pczt = low_level_signer::Signer::new(pczt)
            .sign_ironwood_with(|_, bundle, _| -> Result<(), SignError> {
                for (index, action) in bundle.actions_mut().iter().enumerate() {
                    // Dummy spends were signed by the IO finalizer; the rest are escrow notes.
                    if action.spend().spend_auth_sig().is_none() {
                        let alpha = action.spend().alpha().ok_or_else(|| {
                            SignError::Other(anyhow!("action {index} has no alpha"))
                        })?;
                        spends.push(SpendAuthItem {
                            action_index: index,
                            alpha: pasta_repr(&alpha),
                        });
                    }
                }
                Ok(())
            })
            .map_err(anyhow::Error::from)?
            .finish();
        Ok(UnsignedPayment {
            pczt,
            sighash,
            spends,
        })
    }

    /// Extract the fully signed transaction and store it in the escrow wallet.
    pub fn finalize(&mut self, pczt: Pczt) -> anyhow::Result<TxId> {
        extract_and_store_transaction_from_pczt::<_, ()>(
            self.db_mut(),
            &SystemClock,
            &mut rng(),
            pczt,
            None,
            Some(&VerifyingKey::build(circuit())),
        )
        .map_err(|e| anyhow!("extracting transaction: {e:?}"))
    }

    /// The raw bytes of a stored transaction, ready to broadcast with [`send_raw`].
    pub fn raw_transaction(&self, txid: TxId) -> anyhow::Result<RawTransaction> {
        let tx = self
            .db()
            .get_transaction(txid)
            .map_err(|e| anyhow!("{e:?}"))?
            .context("transaction not in wallet")?;
        let mut raw = RawTransaction::default();
        tx.write(&mut raw.data)?;
        Ok(raw)
    }
}

fn pasta_repr(scalar: &pasta_curves::Fq) -> [u8; 32] {
    use pasta_curves::group::ff::PrimeField;
    scalar.to_repr()
}

/// Apply trustee signatures (64-byte RedPallas `SpendAuth`) to their actions.
/// Each signature is checked against the action's `rk` before it is applied.
pub fn apply_signatures(
    pczt: Pczt,
    sighash: [u8; 32],
    signatures: &[(usize, [u8; 64])],
) -> anyhow::Result<Pczt> {
    Ok(low_level_signer::Signer::new(pczt)
        .sign_ironwood_with(|_, bundle, _| -> Result<(), SignError> {
            let actions = bundle.actions_mut();
            for (index, sig) in signatures {
                let action = actions
                    .get_mut(*index)
                    .ok_or_else(|| SignError::Other(anyhow!("no action {index}")))?;
                action
                    .apply_signature(
                        sighash,
                        redpallas::Signature::<redpallas::SpendAuth>::from(*sig),
                    )
                    .map_err(|e| {
                        SignError::Other(anyhow!("signature for action {index} rejected: {e:?}"))
                    })?;
            }
            Ok(())
        })
        .map_err(anyhow::Error::from)?
        .finish())
}

impl EscrowWallet {
    /// Height a stored transaction was mined at, if it has been.
    pub fn mined_height(&self, txid: &[u8; 32]) -> anyhow::Result<Option<u32>> {
        let conn = rusqlite::Connection::open_with_flags(
            self.path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let height: Option<Option<u32>> = conn
            .query_row(
                "SELECT mined_height FROM transactions WHERE txid = ?1",
                [&txid[..]],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        Ok(height.flatten())
    }
}

/// Serialize a PCZT for storage between signing rounds.
pub fn pczt_to_bytes(pczt: Pczt) -> anyhow::Result<Vec<u8>> {
    pczt.serialize()
        .map_err(|e| anyhow!("encoding PCZT: {e:?}"))
}

pub fn pczt_from_bytes(bytes: &[u8]) -> anyhow::Result<Pczt> {
    Pczt::parse(bytes).map_err(|e| anyhow!("bad PCZT: {e:?}"))
}

/// Broadcast a raw transaction. Kept separate from the wallet so no wallet
/// borrow (and its SQLite connection) is held across the network call.
pub async fn send_raw(client: &mut Client, raw: RawTransaction) -> anyhow::Result<()> {
    let response = client.send_transaction(raw).await?.into_inner();
    if response.error_code != 0 {
        return Err(anyhow!(
            "broadcast failed ({}): {}",
            response.error_code,
            response.error_message
        ));
    }
    Ok(())
}
