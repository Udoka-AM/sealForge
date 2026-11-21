//! Refund recipients (security review D1, second part).
//!
//! [`crate::plan`] proves a refund batch pays only standard-size notes within
//! what the auction owes. This module ties every one of those notes to a bid:
//!
//! - The coordinator says which bid each note refunds ([`RefundClaim`]).
//! - The trustee reads each of those bids from the chain itself, through a
//!   lightwallet server it chose, decrypted with the escrow's viewing key
//!   ([`BidOnChain`]). The bid's memo names its refund address.
//! - A note must go to exactly that address, the bid must match the published
//!   result (or, if the result doesn't list it, be refunded in full, as late
//!   and invalid payments are), and across every batch this trustee has signed
//!   a bid may not get more notes than its refund splits into.
//!
//! Double payment across batches: any two of three trustees sign a batch, and
//! any two pairs of three share a trustee. So if every trustee refuses to sign
//! a note it already signed in a batch that is (or may still be) on chain, no
//! note can be paid twice. [`signed_before`] works that out from the trustee's
//! own ledger.

use std::collections::BTreeMap;

use sealforge_core::{refunds::split_refund, Zatoshi};
use serde::{Deserialize, Serialize};

use crate::plan::Refusal;

/// A payment to the escrow, by transaction id and output (action) index.
pub type BidRef = ([u8; 32], u32);

/// The coordinator's statement that one refund note pays back one bid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundClaim {
    #[serde(with = "hex::serde")]
    pub bid_txid: [u8; 32],
    pub bid_output_index: u32,
    pub value_zat: u64,
}

impl RefundClaim {
    pub fn bid(&self) -> BidRef {
        (self.bid_txid, self.bid_output_index)
    }
}

/// A bid as the trustee read it from the chain: the payment's value and the
/// memo it carried, decrypted with the escrow's incoming viewing key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BidOnChain {
    pub amount_zat: u64,
    /// The memo's refund address and commitment. `None` if the memo isn't a
    /// readable bid memo: such a payment has nowhere to be refunded to.
    pub refund_ua: Option<String>,
    #[serde(default, with = "hex::serde")]
    pub commitment: [u8; 32],
}

/// What the published result says about one bid it lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultBid {
    #[serde(with = "hex::serde")]
    pub commitment: [u8; 32],
    pub refund_due_zat: u64,
}

/// One batch this trustee signed: its transaction id, the last height it can
/// be mined at, and the refund notes it paid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    #[serde(with = "hex::serde")]
    pub batch_txid: [u8; 32],
    pub expiry_height: u32,
    pub notes: Vec<RefundClaim>,
    /// Payouts: the protocol fee this batch paid (issue #74). One fee per
    /// auction: a later payout may not pay it again while this one counts.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fee_zat: u64,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

/// Whether this trustee already signed a payout that pays the fee and that is
/// (or may still be) on chain, other than `this_txid`.
pub fn fee_signed_before(entries: &[(LedgerEntry, bool)], tip: u32, this_txid: &[u8; 32]) -> bool {
    entries.iter().any(|(entry, on_chain)| {
        entry.fee_zat > 0 && &entry.batch_txid != this_txid && still_counts(entry, *on_chain, tip)
    })
}

/// Whether an earlier signed batch still counts against what a bid may be
/// paid. `on_chain`: the trustee's lightwallet server has it. A batch that
/// isn't on chain and has passed its expiry can never be mined: its notes are
/// free to be paid again. Anything else still counts.
pub fn still_counts(entry: &LedgerEntry, on_chain: bool, tip: u32) -> bool {
    on_chain || tip <= entry.expiry_height
}

/// The notes this trustee already signed for each bid, from ledger entries
/// that still count, leaving out the batch being checked (`this_txid`, which
/// is checked again at round 2).
pub fn signed_before(
    entries: &[(LedgerEntry, bool)],
    tip: u32,
    this_txid: &[u8; 32],
) -> BTreeMap<BidRef, Vec<u64>> {
    let mut out: BTreeMap<BidRef, Vec<u64>> = BTreeMap::new();
    for (entry, on_chain) in entries {
        if &entry.batch_txid == this_txid || !still_counts(entry, *on_chain, tip) {
            continue;
        }
        for note in &entry.notes {
            out.entry(note.bid()).or_default().push(note.value_zat);
        }
    }
    out
}

fn refuse(message: impl Into<String>) -> Refusal {
    Refusal {
        class: "payment".to_owned(),
        message: message.into(),
    }
}

/// Is `taken` a sub-multiset of `allowed`?
fn fits(allowed: &[u64], taken: &[u64]) -> bool {
    let mut left: BTreeMap<u64, usize> = BTreeMap::new();
    for v in allowed {
        *left.entry(*v).or_default() += 1;
    }
    for v in taken {
        match left.get_mut(v) {
            Some(n) if *n > 0 => *n -= 1,
            _ => return false,
        }
    }
    true
}

/// Tie a refund batch's payments to bids.
///
/// - `payments`: what the transaction pays, as the shared checks decoded it:
///   (receiver as a unified address, zatoshis).
/// - `claims`: which bid the coordinator says each note refunds.
/// - `bids`: every claimed bid as this trustee read it from the chain.
/// - `result`: the bids the published result lists.
/// - `before`: notes this trustee already signed for each bid ([`signed_before`]).
/// - `receiver`: the Orchard receiver of a unified address, to compare what is
///   paid rather than how the address is written.
pub fn check_refund_recipients(
    payments: &[(String, u64)],
    claims: &[RefundClaim],
    bids: &BTreeMap<BidRef, BidOnChain>,
    result: &BTreeMap<BidRef, ResultBid>,
    before: &BTreeMap<BidRef, Vec<u64>>,
    receiver: impl Fn(&str) -> Option<[u8; 43]>,
) -> Result<(), Refusal> {
    // What each bid may be refunded in total, and where to.
    let mut expected: Vec<([u8; 43], u64)> = Vec::new();
    let mut this_batch: BTreeMap<BidRef, Vec<u64>> = BTreeMap::new();
    for claim in claims {
        let bid_ref = claim.bid();
        let bid = bids.get(&bid_ref).ok_or_else(|| {
            refuse(format!(
                "note of {} zat claims bid {}:{}, which isn't a payment to the escrow on chain",
                claim.value_zat,
                hex::encode(claim.bid_txid),
                claim.bid_output_index
            ))
        })?;
        let refund_ua = bid.refund_ua.as_deref().ok_or_else(|| {
            refuse(format!(
                "bid {}:{} has no readable refund address; it goes to manual review, not a batch",
                hex::encode(claim.bid_txid),
                claim.bid_output_index
            ))
        })?;
        let to = receiver(refund_ua).ok_or_else(|| {
            refuse(format!(
                "bid {}:{}'s refund address has no Orchard receiver",
                hex::encode(claim.bid_txid),
                claim.bid_output_index
            ))
        })?;
        expected.push((to, claim.value_zat));
        this_batch.entry(bid_ref).or_default().push(claim.value_zat);
    }

    for (bid_ref, notes) in &this_batch {
        let bid = &bids[bid_ref];
        // A bid the result lists is refunded what the result says, and only if
        // it is the bid the result committed to. Anything else paid to the
        // escrow (late, or not a valid bid) is refunded in full.
        let due = match result.get(bid_ref) {
            Some(listed) if listed.commitment != bid.commitment => {
                return Err(refuse(format!(
                    "bid {}:{} on chain doesn't carry the commitment the published result lists",
                    hex::encode(bid_ref.0),
                    bid_ref.1
                )))
            }
            Some(listed) => listed.refund_due_zat,
            None => bid.amount_zat,
        };
        let allowed: Vec<u64> = split_refund(Zatoshi(due)).iter().map(|z| z.0).collect();
        let mut taken = before.get(bid_ref).cloned().unwrap_or_default();
        taken.extend(notes);
        if !fits(&allowed, &taken) {
            return Err(refuse(format!(
                "bid {}:{} is owed {due} zat as notes {allowed:?}; with what you already signed, this batch would pay {taken:?}",
                hex::encode(bid_ref.0),
                bid_ref.1
            )));
        }
    }

    // The transaction pays exactly the claimed notes, each to its bid's
    // refund address: same receivers, same values, nothing more or less.
    let mut paid: Vec<([u8; 43], u64)> = Vec::new();
    for (address, zat) in payments {
        let to = receiver(address)
            .ok_or_else(|| refuse(format!("payment to {address} has no Orchard receiver")))?;
        paid.push((to, *zat));
    }
    paid.sort_unstable();
    expected.sort_unstable();
    if paid != expected {
        return Err(refuse(
            "the transaction's refund outputs aren't the claimed notes, each to its bid's refund address",
        ));
    }
    Ok(())
}

impl BidOnChain {
    /// A bid from a decrypted payment to the escrow: its value, and the
    /// refund address and commitment in its memo if the memo is a bid memo.
    pub fn from_payment(amount_zat: u64, memo: Option<&str>) -> Self {
        let memo = memo.and_then(|m| sealforge_core::memo::BidMemo::decode(m).ok());
        BidOnChain {
            amount_zat,
            refund_ua: memo.as_ref().map(|m| m.refund_ua.clone()),
            commitment: memo.map(|m| m.commitment).unwrap_or_default(),
        }
    }
}

/// Everything a trustee checks about a refund batch once it has read the
/// chain, the same in the CLI and the browser: every note goes to its bid's
/// refund address, within what the bid is owed, counting what this trustee
/// signed before. Returns the ledger entry to record before the shares leave.
///
/// - `pczt`, `verified`: the batch's transaction and what [`crate::verify_batch`] found.
/// - `claims`: which bid each note refunds, as the coordinator says.
/// - `bids`: those bids as this trustee read them from the chain.
/// - `listed`: the published result's bids.
/// - `ledger`: what this trustee signed before, each with whether its
///   lightwallet server has that transaction; `tip`: that server's chain tip.
pub fn check_refund_batch(
    pczt: &[u8],
    verified: &crate::VerifiedBatch,
    claims: &[RefundClaim],
    bids: &BTreeMap<BidRef, BidOnChain>,
    listed: &BTreeMap<BidRef, ResultBid>,
    ledger: &[(LedgerEntry, bool)],
    tip: u32,
) -> Result<LedgerEntry, Refusal> {
    let batch_txid = crate::batch_txid(pczt)?;
    let before = signed_before(ledger, tip, &batch_txid);
    check_refund_recipients(
        &verified.payments,
        claims,
        bids,
        listed,
        &before,
        crate::orchard_receiver,
    )?;
    Ok(LedgerEntry {
        batch_txid,
        expiry_height: verified.expiry_height,
        notes: claims.to_vec(),
        fee_zat: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(i: u8) -> [u8; 43] {
        [i; 43]
    }

    /// Addresses in these tests are "ua<N>", with receiver [N; 43].
    fn receiver(address: &str) -> Option<[u8; 43]> {
        address.strip_prefix("ua")?.parse::<u8>().ok().map(r)
    }

    fn bid(i: u8) -> BidRef {
        ([i; 32], 0)
    }

    struct World {
        bids: BTreeMap<BidRef, BidOnChain>,
        result: BTreeMap<BidRef, ResultBid>,
    }

    /// Bid 1 lost 0.03 ZEC (refund 3,000,000 = 2,000,000 + 1,000,000), bid 2
    /// won and is refunded 500,000, bid 3 arrived late with 100,000.
    fn world() -> World {
        let mut bids = BTreeMap::new();
        let mut result = BTreeMap::new();
        for (i, amount, due) in [
            (1u8, 3_000_000u64, Some(3_000_000u64)),
            (2, 2_500_000, Some(500_000)),
            (3, 100_000, None),
        ] {
            bids.insert(
                bid(i),
                BidOnChain {
                    amount_zat: amount,
                    refund_ua: Some(format!("ua{i}")),
                    commitment: [i; 32],
                },
            );
            if let Some(due) = due {
                result.insert(
                    bid(i),
                    ResultBid {
                        commitment: [i; 32],
                        refund_due_zat: due,
                    },
                );
            }
        }
        World { bids, result }
    }

    fn claim(i: u8, value: u64) -> RefundClaim {
        RefundClaim {
            bid_txid: [i; 32],
            bid_output_index: 0,
            value_zat: value,
        }
    }

    fn check(
        w: &World,
        payments: &[(&str, u64)],
        claims: &[RefundClaim],
        before: &BTreeMap<BidRef, Vec<u64>>,
    ) -> Result<(), Refusal> {
        let payments: Vec<(String, u64)> =
            payments.iter().map(|(a, v)| (a.to_string(), *v)).collect();
        check_refund_recipients(&payments, claims, &w.bids, &w.result, before, receiver)
    }

    #[test]
    fn a_bid_payment_reads_its_memo() {
        let memo = sealforge_core::memo::BidMemo {
            auction_code: "7K3QX9MZ2P".into(),
            commitment: [7; 32],
            refund_ua: "utest1refund".into(),
        }
        .encode()
        .unwrap();
        let bid = BidOnChain::from_payment(5_000_000, Some(&format!("{memo}\0\0")));
        assert_eq!(bid.amount_zat, 5_000_000);
        assert_eq!(bid.refund_ua.as_deref(), Some("utest1refund"));
        assert_eq!(bid.commitment, [7; 32]);
        // Not a bid memo, or none: nowhere to refund to.
        assert_eq!(BidOnChain::from_payment(1, Some("hello")).refund_ua, None);
        assert_eq!(BidOnChain::from_payment(1, None).refund_ua, None);
    }

    #[test]
    fn honest_refunds_pass() {
        let w = world();
        let claims = [
            claim(1, 2_000_000),
            claim(1, 1_000_000),
            claim(2, 500_000),
            claim(3, 100_000),
        ];
        let payments = [
            ("ua1", 1_000_000),
            ("ua2", 500_000),
            ("ua1", 2_000_000),
            ("ua3", 100_000),
        ];
        check(&w, &payments, &claims, &BTreeMap::new()).unwrap();
    }

    #[test]
    fn a_note_to_any_other_address_is_refused() {
        let w = world();
        let claims = [claim(1, 2_000_000)];
        // The coordinator claims bid 1 but pays an address of its own.
        let err = check(&w, &[("ua9", 2_000_000)], &claims, &BTreeMap::new()).unwrap_err();
        assert_eq!(err.class, "payment");
    }

    #[test]
    fn notes_swapped_between_bidders_are_refused() {
        let w = world();
        let claims = [claim(1, 1_000_000), claim(2, 500_000)];
        let err = check(
            &w,
            &[("ua2", 1_000_000), ("ua1", 500_000)],
            &claims,
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(err.class, "payment");
    }

    #[test]
    fn more_than_a_bid_is_owed_is_refused() {
        let w = world();
        // Bid 2 is owed 500,000; a second 500,000 note is not in its split.
        let claims = [claim(2, 500_000), claim(2, 500_000)];
        let err = check(
            &w,
            &[("ua2", 500_000), ("ua2", 500_000)],
            &claims,
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(err.message.contains("is owed 500000"), "{err}");
        // A winner can't be refunded its whole payment by claiming it in full.
        let claims = [claim(2, 2_000_000)];
        assert!(check(&w, &[("ua2", 2_000_000)], &claims, &BTreeMap::new()).is_err());
    }

    #[test]
    fn a_note_already_signed_in_a_live_batch_is_refused() {
        let w = world();
        let claims = [claim(1, 2_000_000)];
        let before = BTreeMap::from([(bid(1), vec![2_000_000])]);
        assert!(check(&w, &[("ua1", 2_000_000)], &claims, &before).is_err());
        // The rest of the split is still payable.
        let claims = [claim(1, 1_000_000)];
        check(&w, &[("ua1", 1_000_000)], &claims, &before).unwrap();
    }

    #[test]
    fn a_bid_that_isnt_on_chain_or_doesnt_match_the_result_is_refused() {
        let w = world();
        let err = check(
            &w,
            &[("ua7", 1_000_000)],
            &[claim(7, 1_000_000)],
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(
            err.message
                .contains("isn't a payment to the escrow on chain"),
            "{err}"
        );
        let mut w = world();
        w.bids.get_mut(&bid(1)).unwrap().commitment = [0xee; 32];
        let err = check(
            &w,
            &[("ua1", 1_000_000)],
            &[claim(1, 1_000_000)],
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(err.message.contains("commitment"), "{err}");
    }

    #[test]
    fn an_unclaimed_extra_output_is_refused() {
        let w = world();
        let err = check(
            &w,
            &[("ua1", 1_000_000), ("ua9", 1_000_000)],
            &[claim(1, 1_000_000)],
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(err.class, "payment");
    }

    #[test]
    fn a_fee_counts_once_until_its_payout_expires() {
        let paid = LedgerEntry {
            batch_txid: [6; 32],
            expiry_height: 100,
            notes: vec![],
            fee_zat: 500_000,
        };
        let refund = LedgerEntry {
            batch_txid: [7; 32],
            expiry_height: 100,
            notes: vec![claim(1, 1_000_000)],
            fee_zat: 0,
        };
        let this = [8; 32];
        assert!(!fee_signed_before(&[(refund.clone(), true)], 50, &this));
        assert!(
            fee_signed_before(&[(paid.clone(), false)], 50, &this),
            "may still be mined"
        );
        assert!(
            fee_signed_before(&[(paid.clone(), true)], 500, &this),
            "mined"
        );
        assert!(
            !fee_signed_before(&[(paid.clone(), false)], 500, &this),
            "expired unmined"
        );
        // The same payout asked again (a retry) isn't a second fee.
        assert!(!fee_signed_before(&[(paid, false)], 50, &[6; 32]));
    }

    #[test]
    fn expired_unmined_batches_stop_counting() {
        let entry = LedgerEntry {
            batch_txid: [5; 32],
            expiry_height: 100,
            notes: vec![claim(1, 2_000_000)],
            fee_zat: 0,
        };
        // Mined: counts. Not mined but could still be: counts. Expired: doesn't.
        assert!(still_counts(&entry, true, 500));
        assert!(still_counts(&entry, false, 100));
        assert!(!still_counts(&entry, false, 101));
        let before = signed_before(&[(entry.clone(), false)], 50, &[0; 32]);
        assert_eq!(before[&bid(1)], vec![2_000_000]);
        assert!(signed_before(&[(entry.clone(), false)], 101, &[0; 32]).is_empty());
        // The batch being checked again doesn't count against itself.
        assert!(signed_before(&[(entry, true)], 50, &[5; 32]).is_empty());
    }
}
