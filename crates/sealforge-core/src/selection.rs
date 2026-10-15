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

use crate::{
    memo::{BidMemo, RevealMemo},
    receipt::price_commitment,
    result::ScannedBid,
    Zatoshi,
};

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
    /// Commit-reveal: no valid reveal arrived in the reveal window.
    NotRevealed,
}

impl InvalidReason {
    /// Stored in `bids.invalid_reason`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadMemo => "bad_memo",
            Self::WrongAuction => "wrong_auction",
            Self::BeforeOpen => "before_open",
            Self::DuplicateCommitment => "duplicate_commitment",
            Self::NotRevealed => "not_revealed",
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

// ---------------------------------------------------------------------------
// Commit-reveal mode (PRD F13, task 4.6).
//
// During bidding, a deposit's memo commits to a hidden price (see
// `receipt::price_commitment`) and its amount is only a budget. After the
// bidding close, each bidder sends a reveal (`SF1R:` memo) to the escrow. Both
// are on chain, so the auditor applies exactly these rules too:
// 1. Deposits are classified as in deposit mode, except the reserve applies to
//    the revealed price, not the deposit.
// 2. A reveal counts only if it arrives after `close_height` and at or before
//    `reveal_close_height`, names a valid deposit's commitment, opens it
//    (same code, the deposit's refund address, price and salt), and the price
//    is between 1 zatoshi and the deposit amount.
// 3. The earliest counting reveal per deposit wins (height, txid, output index).
// 4. A deposit with no counting reveal is `Invalid(NotRevealed)` and refunded.

/// A reveal payment to the escrow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealPayment {
    pub txid: [u8; 32],
    pub output_index: u32,
    pub block_height: u32,
    pub amount: Zatoshi,
    pub memo: RevealMemo,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevealWindow {
    pub bid: BidWindow,
    /// Last height at which reveals count.
    pub reveal_close_height: u32,
}

/// One deposit's outcome in commit-reveal mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revealed {
    pub class: BidClass,
    /// The revealed price, when a reveal counted.
    pub price: Option<Zatoshi>,
    /// Index into the reveals of the reveal that counted.
    pub reveal: Option<usize>,
}

/// Classify deposits and match reveals. Outputs are in input order.
pub fn classify_commit_reveal(
    window: &RevealWindow,
    deposits: &[Payment],
    reveals: &[RevealPayment],
) -> Vec<Revealed> {
    // Deposit-level rules, with the reserve deferred to the revealed price.
    let unreserved = BidWindow {
        reserve: Zatoshi(0),
        ..window.bid.clone()
    };
    let classes = classify(&unreserved, deposits);
    let mut by_commitment: BTreeMap<[u8; 32], usize> = BTreeMap::new();
    for (i, (class, d)) in classes.iter().zip(deposits).enumerate() {
        if *class == BidClass::Valid {
            by_commitment.insert(d.memo.as_ref().expect("valid implies a memo").commitment, i);
        }
    }

    let mut order: Vec<usize> = (0..reveals.len()).collect();
    order.sort_by_key(|&i| {
        (
            reveals[i].block_height,
            reveals[i].txid,
            reveals[i].output_index,
        )
    });
    let mut matched: Vec<Option<(Zatoshi, usize)>> = vec![None; deposits.len()];
    for i in order {
        let r = &reveals[i];
        if r.block_height <= window.bid.close_height
            || r.block_height > window.reveal_close_height
            || r.memo.auction_code != window.bid.auction_code
        {
            continue;
        }
        let Some(&d) = by_commitment.get(&r.memo.commitment) else {
            continue;
        };
        if matched[d].is_some() {
            continue;
        }
        let deposit = &deposits[d];
        let refund_ua = &deposit
            .memo
            .as_ref()
            .expect("valid implies a memo")
            .refund_ua;
        let opens = price_commitment(
            &window.bid.auction_code,
            refund_ua,
            r.memo.price_zat,
            &r.memo.salt,
        ) == r.memo.commitment;
        if opens && r.memo.price_zat >= 1 && r.memo.price_zat <= deposit.amount.0 {
            matched[d] = Some((Zatoshi(r.memo.price_zat), i));
        }
    }

    classes
        .into_iter()
        .zip(matched)
        .map(|(class, m)| match (class, m) {
            (BidClass::Valid, Some((price, reveal))) => Revealed {
                class: if price < window.bid.reserve {
                    BidClass::UnderReserve
                } else {
                    BidClass::Valid
                },
                price: Some(price),
                reveal: Some(reveal),
            },
            (BidClass::Valid, None) => Revealed {
                class: BidClass::Invalid(InvalidReason::NotRevealed),
                price: None,
                reveal: None,
            },
            (other, _) => Revealed {
                class: other,
                price: None,
                reveal: None,
            },
        })
        .collect()
}

/// Commit-reveal counterpart of [`included_bids`]: each bid's amount is its
/// revealed price, so clearing and the Merkle tree work unchanged.
pub fn included_bids_commit_reveal(
    window: &RevealWindow,
    deposits: &[Payment],
    reveals: &[RevealPayment],
) -> Vec<ScannedBid> {
    classify_commit_reveal(window, deposits, reveals)
        .into_iter()
        .zip(deposits)
        .filter(|(r, _)| r.class.is_included())
        .map(|(r, d)| ScannedBid {
            txid: d.txid,
            output_index: d.output_index,
            block_height: d.block_height,
            amount: r.price.expect("included reveals have a price"),
            commitment: d
                .memo
                .as_ref()
                .expect("included bids have a memo")
                .commitment,
        })
        .collect()
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

    mod commit_reveal {
        use super::*;
        use crate::receipt::price_commitment;

        const REFUND: &str = "utest1refund";

        fn deposit(tx: u8, height: u32, deposit: u64, price: u64) -> (Payment, RevealMemo) {
            let salt = [tx; 32];
            let c = price_commitment(CODE, REFUND, price, &salt);
            (
                Payment {
                    txid: [tx; 32],
                    output_index: 0,
                    block_height: height,
                    amount: Zatoshi(deposit),
                    memo: Some(BidMemo {
                        auction_code: CODE.into(),
                        commitment: c,
                        refund_ua: REFUND.into(),
                    }),
                },
                RevealMemo {
                    auction_code: CODE.into(),
                    commitment: c,
                    price_zat: price,
                    salt,
                },
            )
        }

        fn reveal(tx: u8, height: u32, memo: RevealMemo) -> RevealPayment {
            RevealPayment {
                txid: [tx; 32],
                output_index: 0,
                block_height: height,
                amount: Zatoshi(10_000),
                memo,
            }
        }

        fn rwindow() -> RevealWindow {
            RevealWindow {
                bid: window(), // open 100, close 200, reserve 1_000
                reveal_close_height: 300,
            }
        }

        #[test]
        fn reveals_set_prices_and_unrevealed_deposits_are_refunded() {
            let (a, ra) = deposit(1, 150, 100_000_000, 42_000_000);
            let (b, _) = deposit(2, 160, 50_000_000, 30_000_000);
            let (c, rc) = deposit(3, 170, 10_000_000, 500); // below the 1,000 reserve
            let out = classify_commit_reveal(
                &rwindow(),
                &[a.clone(), b, c],
                &[reveal(10, 250, ra), reveal(11, 251, rc)],
            );
            assert_eq!(out[0].class, BidClass::Valid);
            assert_eq!(out[0].price, Some(Zatoshi(42_000_000)));
            assert_eq!(out[0].reveal, Some(0));
            assert_eq!(out[1].class, BidClass::Invalid(InvalidReason::NotRevealed));
            assert_eq!(out[2].class, BidClass::UnderReserve);

            let bids = included_bids_commit_reveal(&rwindow(), &[a], &[]);
            assert!(bids.is_empty(), "nothing counts without a reveal");
        }

        #[test]
        fn reveals_must_open_the_commitment_within_the_window_and_budget() {
            let (a, ra) = deposit(1, 150, 40_000_000, 42_000_000); // price above deposit
            let (b, rb) = deposit(2, 150, 50_000_000, 30_000_000);
            let mut wrong_price = rb.clone();
            wrong_price.price_zat = 29_000_000;
            let cases = [
                (reveal(10, 250, ra), "price above the deposit"),
                (reveal(11, 200, rb.clone()), "before the bidding close"),
                (reveal(12, 301, rb.clone()), "after the reveal close"),
                (reveal(13, 250, wrong_price), "doesn't open the commitment"),
            ];
            for (r, why) in cases {
                let out = classify_commit_reveal(&rwindow(), &[a.clone(), b.clone()], &[r]);
                assert!(
                    out.iter().all(|o| o.price.is_none()),
                    "reveal should not count: {why}"
                );
            }
        }

        #[test]
        fn earliest_counting_reveal_wins() {
            let (a, ra) = deposit(1, 150, 100_000_000, 42_000_000);
            let mut bad = ra.clone();
            bad.price_zat = 1; // doesn't open: skipped, doesn't block the good one
            let out = classify_commit_reveal(
                &rwindow(),
                &[a],
                &[
                    reveal(12, 260, ra.clone()),
                    reveal(10, 240, bad),
                    reveal(11, 250, ra),
                ],
            );
            assert_eq!(
                out[0].reveal,
                Some(2),
                "height 250 beats 260; 240 didn't open"
            );
        }
    }
}
