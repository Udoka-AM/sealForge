//! sealForge's own rule on top of the shared checks: every payment in an
//! escrow transaction is one the auction owes.
//!
//! The shared checks ([`crate::frost_zec::check`]) prove what the transaction
//! does: which outputs leave the escrow, for how much, and that nothing is
//! hidden. This module decides whether those payments are allowed for the kind
//! of batch the coordinator says it is:
//!
//! - a consolidation pays no one: every output returns to the escrow;
//! - a payout pays exactly one output, to the organiser's payout address;
//! - a refund pays standard-size notes ({1, 2, 5} × 10^k zat, as
//!   `sealforge_core::refunds::split_refund` makes them), no more of them than
//!   a batch holds, and no more in total than the published result owes.
//!
//! What this does not check yet (security review D1, second part): that each
//! refund note goes to the address in its bidder's memo, and that no note is
//! paid twice across batches. Both need the bid transactions from the chain.

use orchard::keys::FullViewingKey;
use sealforge_core::{denominations::is_canonical, refunds::RefundPolicy, Zatoshi};
use serde::{Deserialize, Serialize};
use zcash_address::unified::{self, Container, Encoding};
use zcash_protocol::consensus::NetworkType;

use crate::frost_zec::{check, pczt_io};

/// What the coordinator says a batch is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchKind {
    Refund,
    Payout,
    Consolidate,
}

impl BatchKind {
    pub fn parse(kind: &str) -> Option<Self> {
        match kind {
            "refund" => Some(Self::Refund),
            "payout" => Some(Self::Payout),
            "consolidate" => Some(Self::Consolidate),
            _ => None,
        }
    }
}

/// What a trustee knows about the batch beyond the PCZT itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchRules {
    pub kind: BatchKind,
    /// Refunds: the total the published result owes bidders, across all
    /// batches. A single batch may not pay more.
    #[serde(default)]
    pub refund_total_due_zat: Option<u64>,
    /// Payouts: the organiser's payout address.
    #[serde(default)]
    pub payout_ua: Option<String>,
    /// Payouts: sealForge's fee, when the auction has one ([`fee_rule`]). A
    /// payout may then carry one more output, to exactly this address and amount.
    #[serde(default)]
    pub fee: Option<FeeRule>,
    /// Payouts: this trustee already signed a payout that pays the fee, and it
    /// is (or may still be) on chain. Another fee output is refused.
    #[serde(default)]
    pub fee_already_signed: bool,
}

/// The fee a payout may pay: to this address, exactly this amount.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeRule {
    pub ua: String,
    pub zat: u64,
}

/// The fee rule for a payout, from the published result: the fee recomputed
/// from every bid's `price_paid_zat` at the published rate
/// (`sealforge_core::fee`), which must equal the fee the result states.
/// `None` when the auction has no fee.
pub fn fee_rule(
    fee_bps: u16,
    fee_ua: Option<&str>,
    published_fee_zat: u64,
    prices_paid: impl IntoIterator<Item = u64>,
) -> Result<Option<FeeRule>, Refusal> {
    let proceeds: u64 = prices_paid.into_iter().sum();
    let fee = sealforge_core::fee::protocol_fee_zat(proceeds, fee_bps);
    if fee != published_fee_zat {
        return Err(refuse(
            "payment",
            format!(
                "the result states a fee of {published_fee_zat} zat, but {fee_bps} bps of the \
                 {proceeds} zat paid is {fee}"
            ),
        ));
    }
    if fee == 0 {
        return Ok(None);
    }
    let ua =
        fee_ua.ok_or_else(|| refuse("payment", "the result states a fee but no fee address"))?;
    Ok(Some(FeeRule {
        ua: ua.to_owned(),
        zat: fee,
    }))
}

/// Why a trustee refuses, with the shared refusal class
/// (frost-zec `vectors/tamper/classes.md`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub class: String,
    pub message: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused ({}): {}", self.class, self.message)
    }
}

impl std::error::Error for Refusal {}

fn refuse(class: &str, message: impl Into<String>) -> Refusal {
    Refusal {
        class: class.to_owned(),
        message: message.into(),
    }
}

/// What the trustee checked, to show before it signs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedBatch {
    /// The sighash computed here from the PCZT. Sign this, nothing else.
    #[serde(with = "hex::serde")]
    pub sighash: [u8; 32],
    /// The transaction line by line, as the shared checks show it.
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    /// Payments leaving the escrow: (receiver as a unified address, zatoshis).
    pub payments: Vec<(String, u64)>,
    pub fee_zat: u64,
    pub expiry_height: u32,
    /// Payouts: the protocol fee this transaction pays (0 if none).
    #[serde(default)]
    pub protocol_fee_zat: u64,
}

/// The Orchard receiver of a unified address, to compare payments by what is
/// actually paid rather than by how the address is written.
pub fn orchard_receiver(ua: &str) -> Option<[u8; 43]> {
    let (_, _, address) = unified::Address::decode(ua.trim()).ok()?;
    address.items().into_iter().find_map(|item| match item {
        unified::Receiver::Orchard(bytes) => Some(bytes),
        _ => None,
    })
}

/// The id the transaction will have on chain, computed from the unsigned PCZT
/// (ZIP 244: the txid doesn't cover signatures). What a trustee's ledger keys
/// its signed batches by.
pub fn batch_txid(pczt_bytes: &[u8]) -> Result<[u8; 32], Refusal> {
    use zcash_primitives::transaction::txid::{to_txid, TxIdDigester};
    let pczt = pczt_io::parse(pczt_bytes).map_err(|e| refuse("structure", e.to_string()))?;
    let effects = pczt
        .into_effects()
        .map_err(|e| refuse("structure", format!("incomplete transaction: {e:?}")))?;
    let txid = to_txid(
        effects.version(),
        effects.consensus_branch_id(),
        &effects.digest(TxIdDigester),
    );
    Ok(*txid.as_ref())
}

/// The escrow spends still waiting for a signature: (action index, `alpha`).
fn spends_to_sign(pczt: &pczt::Pczt) -> Result<Vec<(usize, [u8; 32])>, Refusal> {
    use ff::PrimeField;
    let mut found = Vec::new();
    pczt::roles::low_level_signer::Signer::new(pczt.clone())
        .sign_ironwood_with(|_, bundle, _| -> Result<(), pczt_io::TxError> {
            for (index, action) in bundle.actions().iter().enumerate() {
                if action.spend().spend_auth_sig().is_none() {
                    if let Some(alpha) = action.spend().alpha() {
                        found.push((index, alpha.to_repr()));
                    }
                }
            }
            Ok(())
        })
        .map_err(|e| refuse("structure", e.to_string()))?;
    Ok(found)
}

/// Check an escrow PCZT before co-signing it. `presented_sighash` and
/// `presented_spends` (action index, `alpha`) are what the coordinator asked
/// to sign; both must match the PCZT. `now` is the trustee's clock (Unix
/// seconds), which bounds the expiry.
pub fn verify_batch(
    pczt_bytes: &[u8],
    fvk: &FullViewingKey,
    network: NetworkType,
    now: u64,
    presented_sighash: &[u8; 32],
    presented_spends: &[(usize, [u8; 32])],
    rules: &BatchRules,
) -> Result<VerifiedBatch, Refusal> {
    let pczt = pczt_io::parse(pczt_bytes).map_err(|e| refuse("structure", e.to_string()))?;
    let report = check::analyze_at(&pczt, fvk, network, now).map_err(|e| {
        let message = e.to_string();
        refuse(crate::refusal_class(&message), message)
    })?;
    let sighash = pczt_io::sighash(&pczt).map_err(|e| refuse("structure", e.to_string()))?;
    if &sighash != presented_sighash {
        return Err(refuse(
            "sighash",
            format!(
                "the coordinator asked to sign {}, but this transaction's sighash is {}",
                hex::encode(presented_sighash),
                hex::encode(sighash)
            ),
        ));
    }
    // The randomizers the coordinator asks this trustee to sign with must be
    // the PCZT's, spend for spend.
    if spends_to_sign(&pczt)? != presented_spends {
        return Err(refuse(
            "structure",
            "the spends and randomizers to sign don't match the transaction",
        ));
    }
    let payments: Vec<(String, u64)> = report
        .payments
        .iter()
        .map(|p| (p.address.clone(), p.zat))
        .collect();

    let mut protocol_fee_zat = 0;
    match rules.kind {
        BatchKind::Consolidate => {
            if !payments.is_empty() {
                return Err(refuse(
                    "payment",
                    format!(
                        "a consolidation must return every note to the escrow, but it pays {} output(s) out",
                        payments.len()
                    ),
                ));
            }
        }
        BatchKind::Payout => {
            let expected = rules.payout_ua.as_deref().ok_or_else(|| {
                refuse("payment", "no payout address to check the payout against")
            })?;
            let to = |address: &str, want: &str| {
                orchard_receiver(address).is_some()
                    && orchard_receiver(address) == orchard_receiver(want)
            };
            match payments.as_slice() {
                [(address, _)] if to(address, expected) => {}
                [(address, _)] => {
                    return Err(refuse(
                        "payment",
                        format!("the payout goes to {address}, not the payout address {expected}"),
                    ))
                }
                // The organiser's payout plus sealForge's fee (issue #74).
                [_, _] => {
                    let fee = rules.fee.as_ref().ok_or_else(|| {
                        refuse(
                            "payment",
                            "a payout pays exactly one output: this auction has no fee",
                        )
                    })?;
                    let fee_out = payments
                        .iter()
                        .position(|(a, v)| to(a, &fee.ua) && *v == fee.zat);
                    let Some(i) = fee_out else {
                        return Err(refuse(
                            "payment",
                            format!(
                                "neither output is the fee the published result implies ({} zat to {})",
                                fee.zat, fee.ua
                            ),
                        ));
                    };
                    let (payout_address, _) = &payments[1 - i];
                    if !to(payout_address, expected) {
                        return Err(refuse(
                            "payment",
                            format!(
                                "the payout goes to {payout_address}, not the payout address {expected}"
                            ),
                        ));
                    }
                    if rules.fee_already_signed {
                        return Err(refuse(
                            "payment",
                            "the fee was already paid in a payout you signed",
                        ));
                    }
                    protocol_fee_zat = fee.zat;
                }
                _ => {
                    return Err(refuse(
                        "payment",
                        format!(
                            "a payout pays one output, or two with the fee, not {}",
                            payments.len()
                        ),
                    ))
                }
            }
        }
        BatchKind::Refund => {
            let max = RefundPolicy::default().max_outputs_per_batch;
            if payments.is_empty() || payments.len() > max {
                return Err(refuse(
                    "payment",
                    format!(
                        "a refund batch pays 1 to {max} notes, not {}",
                        payments.len()
                    ),
                ));
            }
            if let Some((address, zat)) = payments
                .iter()
                .find(|(_, zat)| !is_canonical(Zatoshi(*zat)))
            {
                return Err(refuse(
                    "payment",
                    format!("{zat} zat to {address} is not a standard refund note"),
                ));
            }
            let total: u64 = payments.iter().map(|(_, zat)| zat).sum();
            match rules.refund_total_due_zat {
                Some(due) if total <= due => {}
                Some(due) => {
                    return Err(refuse(
                        "payment",
                        format!("the batch refunds {total} zat, more than the {due} zat the result owes in total"),
                    ))
                }
                None => return Err(refuse("payment", "no published result to check refunds against")),
            }
        }
    }

    Ok(VerifiedBatch {
        sighash,
        lines: report.lines,
        warnings: report.warnings,
        payments,
        fee_zat: report.fee_zat,
        expiry_height: report.expiry_height,
        protocol_fee_zat,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frost_zec::keys;

    fn spends(pczt: &[u8]) -> Vec<(usize, [u8; 32])> {
        spends_to_sign(&pczt_io::parse(pczt).unwrap()).unwrap()
    }

    /// An honest mainnet PCZT from the shared cases: two of the group's notes
    /// pay 400,000 zat to a stranger, with change back to the group.
    fn fixture() -> (Vec<u8>, FullViewingKey, u64, [u8; 32], String) {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/tamper/two-spends"
        );
        let case: serde_json::Value =
            serde_json::from_slice(&std::fs::read(format!("{dir}/case.json")).unwrap()).unwrap();
        let (_, fvk) = keys::parse_ufvk(case["group"]["ufvk"].as_str().unwrap()).unwrap();
        let sighash: [u8; 32] = hex::decode(case["accepted"]["sighash"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        (
            std::fs::read(format!("{dir}/accepted.pczt")).unwrap(),
            fvk,
            case["clock"].as_u64().unwrap(),
            sighash,
            case["approved"]["payments"][0]["address"]
                .as_str()
                .unwrap()
                .to_owned(),
        )
    }

    fn rules(kind: BatchKind) -> BatchRules {
        BatchRules {
            kind,
            refund_total_due_zat: Some(10_000_000),
            payout_ua: None,
            fee: None,
            fee_already_signed: false,
        }
    }

    #[test]
    fn a_payout_to_the_payout_address_passes() {
        let (pczt, fvk, now, sighash, paid) = fixture();
        let rules = BatchRules {
            payout_ua: Some(paid),
            ..rules(BatchKind::Payout)
        };
        let ok = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &sighash,
            &spends(&pczt),
            &rules,
        )
        .unwrap();
        assert_eq!(ok.sighash, sighash);
        assert_eq!(ok.payments.len(), 1);
        assert_eq!(ok.payments[0].1, 400_000);
    }

    #[test]
    fn a_payout_anywhere_else_is_refused() {
        let (pczt, fvk, now, sighash, _) = fixture();
        let own = keys::encode_address(
            &fvk.address_at(0u32, orchard::keys::Scope::External),
            NetworkType::Main,
        )
        .unwrap();
        let rules = BatchRules {
            payout_ua: Some(own),
            ..rules(BatchKind::Payout)
        };
        let err = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &sighash,
            &spends(&pczt),
            &rules,
        )
        .unwrap_err();
        assert_eq!(err.class, "payment");
    }

    #[test]
    fn a_consolidation_that_pays_out_is_refused() {
        let (pczt, fvk, now, sighash, _) = fixture();
        let err = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &sighash,
            &spends(&pczt),
            &rules(BatchKind::Consolidate),
        )
        .unwrap_err();
        assert_eq!(err.class, "payment");
    }

    #[test]
    fn a_refund_of_a_non_standard_amount_is_refused() {
        // 400,000 zat is not {1, 2, 5} x 10^k: no refund split makes it.
        let (pczt, fvk, now, sighash, _) = fixture();
        let err = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &sighash,
            &spends(&pczt),
            &rules(BatchKind::Refund),
        )
        .unwrap_err();
        assert_eq!(err.class, "payment");
        assert!(err.message.contains("not a standard refund note"), "{err}");
    }

    #[test]
    fn signing_anything_but_the_computed_sighash_is_refused() {
        let (pczt, fvk, now, _, paid) = fixture();
        let rules = BatchRules {
            payout_ua: Some(paid),
            ..rules(BatchKind::Payout)
        };
        let err = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &[7; 32],
            &spends(&pczt),
            &rules,
        )
        .unwrap_err();
        assert_eq!(err.class, "sighash");
    }

    #[test]
    fn signing_with_other_randomizers_is_refused() {
        let (pczt, fvk, now, sighash, paid) = fixture();
        let rules = BatchRules {
            payout_ua: Some(paid),
            ..rules(BatchKind::Payout)
        };
        let mut wrong = spends(&pczt);
        wrong[0].1[0] ^= 1;
        let err = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &sighash,
            &wrong,
            &rules,
        )
        .unwrap_err();
        assert_eq!(err.class, "structure");
    }

    #[test]
    fn the_batch_txid_is_stable_and_bound_to_the_transaction() {
        let (pczt, ..) = fixture();
        let a = batch_txid(&pczt).unwrap();
        assert_eq!(a, batch_txid(&pczt).unwrap());
        let other = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/tamper/fee-above-zip-317/accepted.pczt"
        ))
        .unwrap();
        assert_ne!(a, batch_txid(&other).unwrap());
    }

    #[test]
    fn the_fee_rule_follows_from_the_published_result() {
        let prices = [10_000_000u64, 10_000_000, 0];
        // 2.5% of 0.2 ZEC.
        assert_eq!(
            fee_rule(250, Some("utest1fee"), 500_000, prices).unwrap(),
            Some(FeeRule {
                ua: "utest1fee".into(),
                zat: 500_000
            })
        );
        // No fee: no rule, whatever the address.
        assert_eq!(fee_rule(0, None, 0, prices).unwrap(), None);
        // A result that states another fee than its own bids give is refused.
        assert_eq!(
            fee_rule(250, Some("utest1fee"), 600_000, prices)
                .unwrap_err()
                .class,
            "payment"
        );
        assert_eq!(fee_rule(0, None, 1, prices).unwrap_err().class, "payment");
        // A fee with nowhere to go is refused.
        assert!(fee_rule(250, None, 500_000, prices).is_err());
    }

    #[test]
    fn a_payout_without_the_fee_output_still_passes() {
        // The fee rides in the first payout sweep; a payout without it (a
        // later sweep, or one too small to carry it) is still the payout.
        let (pczt, fvk, now, sighash, paid) = fixture();
        let rules = BatchRules {
            payout_ua: Some(paid),
            fee: Some(FeeRule {
                ua: "u1unused".into(),
                zat: 1,
            }),
            ..rules(BatchKind::Payout)
        };
        let ok = verify_batch(
            &pczt,
            &fvk,
            NetworkType::Main,
            now,
            &sighash,
            &spends(&pczt),
            &rules,
        )
        .unwrap();
        assert_eq!(ok.protocol_fee_zat, 0);
    }

    #[test]
    fn a_pczt_for_another_escrow_is_refused() {
        let (pczt, _, now, sighash, paid) = fixture();
        let other = orchard::keys::FullViewingKey::from(
            &orchard::keys::SpendingKey::from_bytes([9; 32]).unwrap(),
        );
        let rules = BatchRules {
            payout_ua: Some(paid),
            ..rules(BatchKind::Payout)
        };
        let err = verify_batch(
            &pczt,
            &other,
            NetworkType::Main,
            now,
            &sighash,
            &spends(&pczt),
            &rules,
        )
        .unwrap_err();
        assert_eq!(err.class, "foreign-spend");
    }
}
