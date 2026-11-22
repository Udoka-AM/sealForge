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

/// Most escrow notes one transaction may spend. ZIP 218 limits Orchard and
/// Ironwood actions to 330 per block, shared by every transaction in it, and a
/// transaction above the limit can never be mined. Each spent note is at least
/// one action, so this stays well under it (issue #41).
pub const MAX_NOTES_PER_TX: usize = 100;

/// ZIP 218: Orchard and Ironwood actions per block, shared across transactions.
const ZIP_218_ACTIONS_PER_BLOCK: usize = 330;
// Leave room for at least three escrow transactions per block.
const _: () = assert!(MAX_NOTES_PER_TX * 3 <= ZIP_218_ACTIONS_PER_BLOCK);

/// A payment would spend more escrow notes than the cap allows. The caller
/// consolidates notes first ([`EscrowWallet::build_consolidation`]) and retries.
#[derive(Debug)]
pub struct TooManyNotes {
    pub notes: usize,
    pub max: usize,
}

impl std::fmt::Display for TooManyNotes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the payment would spend {} escrow notes, over the limit of {} per transaction",
            self.notes, self.max
        )
    }
}

impl std::error::Error for TooManyNotes {}

/// A proved sweep and how much it sends.
pub struct Sweep {
    pub payment: UnsignedPayment,
    pub amount_zat: u64,
    /// Notes this sweep spends.
    pub notes: usize,
    /// The escrow held more notes than one transaction may spend, so this sweep
    /// took only the oldest ones and another is needed for the rest.
    pub more_left: bool,
    /// The extra output (the protocol fee) is in this sweep. It is left out
    /// when the notes swept can't cover it; the next sweep then pays it.
    pub extra_paid: bool,
}

/// `(commitment tree position, value)` of every note a proposal spends.
fn spent_notes(proposal: &EscrowProposal) -> Vec<(u64, u64)> {
    proposal
        .steps()
        .iter()
        .filter_map(|step| step.shielded_inputs())
        .flat_map(|inputs| inputs.notes().iter())
        .map(|n| {
            (
                u64::from(n.note_commitment_tree_position()),
                u64::from(n.note().value()),
            )
        })
        .collect()
}

/// The amount that spends exactly the `max_notes` oldest of `notes` (the
/// wallet selects oldest first), with `fee` taken out, and how many notes that is.
pub fn oldest_notes_target(notes: &[(u64, u64)], max_notes: usize, fee: u64) -> (u64, usize) {
    let mut sorted = notes.to_vec();
    sorted.sort_unstable();
    let taken = &sorted[..sorted.len().min(max_notes)];
    let total: u64 = taken.iter().map(|(_, v)| v).sum();
    (total.saturating_sub(fee), taken.len())
}

/// ZIP-317 fee for a transaction spending `notes` escrow notes into at most two
/// outputs: 5,000 zat per logical action, at least two.
pub fn sweep_fee(notes: usize) -> u64 {
    5_000 * notes.max(2) as u64
}

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
    /// (building the Ironwood proving key plus the proof itself). Fails with
    /// [`TooManyNotes`] if it would spend more than `max_notes` notes.
    pub fn build_payment(
        &mut self,
        outputs: &[PaymentOut],
        max_notes: usize,
    ) -> anyhow::Result<UnsignedPayment> {
        let proposal = self.propose(outputs).map_err(anyhow::Error::from)?;
        let notes = spent_notes(&proposal).len();
        if notes > max_notes {
            return Err(TooManyNotes {
                notes,
                max: max_notes,
            }
            .into());
        }
        self.prove(&proposal)
    }

    /// The escrow's own internal (change) address. Notes sent here are seen by
    /// the escrow's full viewing key but not by the published UIVK, and the
    /// coordinator never counts them as bids (they arrive as change).
    fn internal_address(&self) -> anyhow::Result<String> {
        use zcash_client_backend::data_api::Account as _;
        let account = self.account()?;
        let ufvk = self
            .db()
            .get_account(account)
            .map_err(|e| anyhow!("{e:?}"))?
            .ok_or_else(|| anyhow!("escrow wallet has no account"))?
            .ufvk()
            .cloned()
            .ok_or_else(|| anyhow!("escrow account has no UFVK"))?;
        let fvk = ufvk
            .orchard()
            .ok_or_else(|| anyhow!("escrow UFVK has no Orchard key"))?;
        let receiver = fvk.address_at(0u32, orchard::keys::Scope::Internal);
        let ua = zcash_keys::address::UnifiedAddress::from_receivers(
            Some(receiver),
            None,
            None,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("no shielded receiver"))?;
        Ok(ua.encode(&self.params()))
    }

    /// Merge the `max_notes` oldest escrow notes into one note at the escrow's
    /// own internal address, so a later payment needs fewer inputs. No funds
    /// leave the escrow except the fee.
    pub fn build_consolidation(&mut self, max_notes: usize) -> anyhow::Result<Sweep> {
        let address = self.internal_address()?;
        self.build_sweep(&address, 0, max_notes, None)
    }

    /// Pay everything in the escrow to `address` except `keep_zat`, net of the
    /// ZIP-317 fee (the organiser payout), spending at most `max_notes` notes.
    /// If the escrow holds more, this sends what its oldest `max_notes` notes
    /// hold and sets [`Sweep::more_left`]; sweep again once it confirms.
    /// Sweep the escrow to `address`, keeping `keep_zat`. With `extra` (the
    /// protocol fee), that output is paid first at its exact amount and
    /// `address` gets the rest; it rides along only if the notes swept cover it.
    pub fn build_sweep(
        &mut self,
        address: &str,
        keep_zat: u64,
        max_notes: usize,
        extra: Option<&PaymentOut>,
    ) -> anyhow::Result<Sweep> {
        let available = self.spendable_zat()?;
        let extra_zat = extra.map_or(0, |e| e.amount_zat);
        // The outputs for a sweep moving `total` out of the escrow: the extra
        // output at its amount, the rest to `address`.
        let outs = |total: u64, with_extra: bool| -> Option<(Vec<PaymentOut>, u64)> {
            let main = if with_extra {
                total.checked_sub(extra_zat)?
            } else {
                total
            };
            if main == 0 {
                return None;
            }
            let mut v = vec![PaymentOut {
                address: address.to_owned(),
                amount_zat: main,
                memo: None,
            }];
            if with_extra {
                v.extend(extra.cloned());
            }
            Some((v, main))
        };
        // Everything: the fee depends on the notes selected, so learn it from
        // the shortfall the proposal reports.
        let mut fee = sweep_fee(2);
        let mut full = None;
        for _ in 0..5 {
            let total = available
                .checked_sub(keep_zat + fee)
                .filter(|a| *a > 0)
                .ok_or_else(|| anyhow!("escrow holds {available} zat: nothing left to sweep"))?;
            let with_extra = extra.is_some() && total > extra_zat;
            let (outputs, main) = outs(total, with_extra)
                .ok_or_else(|| anyhow!("escrow holds {available} zat: nothing left to sweep"))?;
            match self.propose(&outputs) {
                Ok(proposal) => {
                    full = Some((proposal, main, with_extra));
                    break;
                }
                Err(ProposeFail::Insufficient { required, .. }) => {
                    fee = required.saturating_sub(total).max(fee + 5_000);
                }
                Err(ProposeFail::Other(e)) => return Err(e),
            }
        }
        let (proposal, amount, extra_paid) =
            full.ok_or_else(|| anyhow!("couldn't size the sweep"))?;
        let notes = spent_notes(&proposal);
        if notes.len() <= max_notes {
            return Ok(Sweep {
                payment: self.prove(&proposal)?,
                amount_zat: amount,
                notes: notes.len(),
                more_left: false,
                extra_paid,
            });
        }

        // Too many notes for one transaction: send what the oldest `max_notes`
        // hold. The wallet spends oldest first, so a target just under their
        // total selects exactly those. Anything to keep stays in the newer notes.
        let mut fee = sweep_fee(max_notes);
        for _ in 0..10 {
            let (target, _) = oldest_notes_target(&notes, max_notes, fee);
            let total = target.min(available.saturating_sub(keep_zat + fee));
            if total == 0 {
                break;
            }
            let with_extra = extra.is_some() && total > extra_zat;
            let Some((outputs, main)) = outs(total, with_extra) else {
                break;
            };
            match self.propose(&outputs) {
                Ok(proposal) => {
                    let spent = spent_notes(&proposal).len();
                    if spent <= max_notes {
                        return Ok(Sweep {
                            payment: self.prove(&proposal)?,
                            amount_zat: main,
                            notes: spent,
                            more_left: true,
                            extra_paid: with_extra,
                        });
                    }
                    // The fee ran past the oldest notes' total: leave more room.
                    fee += 5_000 * (spent - max_notes) as u64;
                }
                Err(ProposeFail::Insufficient { required, .. }) => {
                    fee = required.saturating_sub(total).max(fee + 5_000);
                }
                Err(ProposeFail::Other(e)) => return Err(e),
            }
        }
        Err(anyhow!(
            "couldn't size a sweep of the oldest {max_notes} of {} escrow notes",
            notes.len()
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_covers_exactly_the_oldest_notes() {
        // Positions out of order on purpose: selection is by age, not input order.
        let notes = [(7, 300_000), (2, 100_000), (5, 200_000), (9, 400_000)];
        let fee = sweep_fee(2);
        assert_eq!(oldest_notes_target(&notes, 2, fee), (300_000 - fee, 2));
        assert_eq!(oldest_notes_target(&notes, 10, fee), (1_000_000 - fee, 4));
        // Notes worth less than the fee leave nothing to send.
        assert_eq!(
            oldest_notes_target(&[(1, 6_000), (2, 6_000)], 2, fee).0,
            2_000
        );
        assert_eq!(oldest_notes_target(&[(1, 5_001)], 1, fee).0, 0);
    }

    #[test]
    fn fees_follow_zip_317() {
        assert_eq!(sweep_fee(0), 10_000);
        assert_eq!(sweep_fee(1), 10_000);
        assert_eq!(sweep_fee(MAX_NOTES_PER_TX), 500_000);
    }
}
