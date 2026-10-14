//! Which escrow payments count as bids (task 3.3).
//!
//! The scanner, the coordinator at close, and the public auditor all classify
//! payments with [`classify`], using only data visible through the published
//! viewing key (memo, amount, block height, txid). That is what lets anyone
//! reproduce the bid set, and therefore the result, from the chain.
//!
//! Rules, in order:
//! 1. The memo must parse as a v1 bid memo for this auction, else `Invalid`.
//! 2. A payment before the auction opened is `Invalid(BeforeOpen)`: the escrow
//!    address was not public yet.
//! 3. A payment after the close height is `Late`.
//! 4. If several payments reuse a commitment, the earliest
//!    (block height, txid, output index) wins and the rest are
//!    `Invalid(DuplicateCommitment)`.
//! 5. A remaining payment below the reserve is `UnderReserve`, otherwise `Valid`.
//!
//! Every non-`Valid` payment is refunded in full. `Valid` and `UnderReserve`
//! bids both go into the Merkle tree (see [`crate::result::compute_result`]),
//! so every bidder can prove their bid was seen.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{memo::BidMemo, result::ScannedBid, Zatoshi};

/// One payment received by the escrow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payment {
    pub txid: [u8; 32],
    pub output_index: u32,
    pub block_height: u32,
    pub amount: Zatoshi,
    /// The decoded memo, or `None` if it was not a valid v1 bid memo.
    pub memo: Option<BidMemo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidReason {
    BadMemo,
    WrongAuction,
    BeforeOpen,
    DuplicateCommitment,
}

impl InvalidReason {
    /// Stored in `bids.invalid_reason`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadMemo => "bad_memo",
            Self::WrongAuction => "wrong_auction",
            Self::BeforeOpen => "before_open",
            Self::DuplicateCommitment => "duplicate_commitment",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BidClass {
    Valid,
    UnderReserve,
    Late,
    Invalid(InvalidReason),
}

impl BidClass {
    /// Whether the bid is part of the result (clearing and Merkle tree).
    pub fn is_included(self) -> bool {
        matches!(self, Self::Valid | Self::UnderReserve)
    }
}

/// The auction parameters classification depends on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BidWindow {
    pub auction_code: String,
    /// First height at which bids count (the height the auction opened at).
    pub open_height: u32,
    /// Last height at which bids count.
    pub close_height: u32,
    pub reserve: Zatoshi,
}

/// Classify every payment. The output is in input order.
pub fn classify(window: &BidWindow, payments: &[Payment]) -> Vec<BidClass> {
    let mut classes: Vec<BidClass> = payments
        .iter()
        .map(|p| match &p.memo {
            None => BidClass::Invalid(InvalidReason::BadMemo),
            Some(m) if m.auction_code != window.auction_code => {
                BidClass::Invalid(InvalidReason::WrongAuction)
            }
            Some(_) if p.block_height < window.open_height => {
                BidClass::Invalid(InvalidReason::BeforeOpen)
            }
            Some(_) if p.block_height > window.close_height => BidClass::Late,
            Some(_) => BidClass::Valid,
        })
        .collect();

    // Earliest payment per commitment wins.
    let mut first: BTreeMap<[u8; 32], usize> = BTreeMap::new();
    for (i, p) in payments.iter().enumerate() {
        if classes[i] != BidClass::Valid {
            continue;
        }
        let commitment = p.memo.as_ref().expect("valid implies a memo").commitment;
        let key = |j: usize| {
            (
                payments[j].block_height,
                payments[j].txid,
                payments[j].output_index,
            )
        };
        match first.get(&commitment) {
            Some(&j) if key(j) <= key(i) => {
                classes[i] = BidClass::Invalid(InvalidReason::DuplicateCommitment)
            }
            Some(&j) => {
                classes[j] = BidClass::Invalid(InvalidReason::DuplicateCommitment);
                first.insert(commitment, i);
            }
            None => {
                first.insert(commitment, i);
            }
        }
    }

    for (class, p) in classes.iter_mut().zip(payments) {
        if *class == BidClass::Valid && p.amount < window.reserve {
            *class = BidClass::UnderReserve;
        }
    }
    classes
}

/// The bids that go into the result, ready for [`crate::result::compute_result`].
pub fn included_bids(window: &BidWindow, payments: &[Payment]) -> Vec<ScannedBid> {
    classify(window, payments)
        .into_iter()
        .zip(payments)
        .filter(|(class, _)| class.is_included())
        .map(|(_, p)| ScannedBid {
            txid: p.txid,
            output_index: p.output_index,
            block_height: p.block_height,
            amount: p.amount,
            commitment: p
                .memo
                .as_ref()
                .expect("included bids have a memo")
                .commitment,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "7K3QX9MZ2P";

    fn window() -> BidWindow {
        BidWindow {
            auction_code: CODE.into(),
            open_height: 100,
            close_height: 200,
            reserve: Zatoshi(1_000),
        }
    }

    fn pay(tx: u8, height: u32, amount: u64, code: &str, commitment: u8) -> Payment {
        Payment {
            txid: [tx; 32],
            output_index: 0,
            block_height: height,
            amount: Zatoshi(amount),
            memo: Some(BidMemo {
                auction_code: code.into(),
                commitment: [commitment; 32],
                refund_ua: "utest1refund".into(),
            }),
        }
    }

    #[test]
    fn classifies_each_rule() {
        let mut bad_memo = pay(9, 150, 5_000, CODE, 9);
        bad_memo.memo = None;
        let payments = [
            pay(1, 150, 5_000, CODE, 1),
            bad_memo,
            pay(2, 150, 5_000, "ZZZZZZZZZZ", 2),
            pay(3, 99, 5_000, CODE, 3),
            pay(4, 201, 5_000, CODE, 4),
            pay(5, 150, 500, CODE, 5),
            pay(6, 100, 5_000, CODE, 6),
            pay(7, 200, 5_000, CODE, 7),
        ];
        assert_eq!(
            classify(&window(), &payments),
            vec![
                BidClass::Valid,
                BidClass::Invalid(InvalidReason::BadMemo),
                BidClass::Invalid(InvalidReason::WrongAuction),
                BidClass::Invalid(InvalidReason::BeforeOpen),
                BidClass::Late,
                BidClass::UnderReserve,
                BidClass::Valid, // open height is inclusive
                BidClass::Valid, // close height is inclusive
            ]
        );
    }

    #[test]
    fn earliest_duplicate_commitment_wins_regardless_of_order() {
        let early = pay(5, 120, 5_000, CODE, 1);
        let late = pay(1, 130, 9_000, CODE, 1);
        let same_block_higher_txid = pay(8, 120, 7_000, CODE, 1);
        let classes = classify(
            &window(),
            &[late.clone(), same_block_higher_txid.clone(), early.clone()],
        );
        assert_eq!(
            classes,
            vec![
                BidClass::Invalid(InvalidReason::DuplicateCommitment),
                BidClass::Invalid(InvalidReason::DuplicateCommitment),
                BidClass::Valid,
            ]
        );
        let classes = classify(&window(), &[early, same_block_higher_txid, late]);
        assert_eq!(classes[0], BidClass::Valid);
    }

    #[test]
    fn late_or_invalid_payments_do_not_block_a_commitment() {
        // A late payment reusing a commitment doesn't invalidate the on-time one.
        let classes = classify(
            &window(),
            &[pay(1, 250, 5_000, CODE, 1), pay(2, 150, 5_000, CODE, 1)],
        );
        assert_eq!(classes, vec![BidClass::Late, BidClass::Valid]);
    }

    #[test]
    fn included_bids_feed_compute_result() {
        let payments = [
            pay(1, 150, 5_000, CODE, 1),
            pay(2, 150, 500, CODE, 2),
            pay(3, 250, 5_000, CODE, 3),
        ];
        let bids = included_bids(&window(), &payments);
        assert_eq!(bids.len(), 2, "valid + under reserve, not late");
        let rules = crate::clearing::AuctionRules {
            mode: crate::clearing::AuctionMode::UniformPrice,
            quantity: 1,
            reserve: window().reserve,
        };
        let result = crate::result::compute_result(&rules, &bids).unwrap();
        assert_eq!(result.clearing.valid_bid_count, 1);
        assert_eq!(result.clearing.outcomes.len(), 2);
    }
}
