//! Clearing engine (Milestone 3, task 3.1).
//!
//! Rules (PRD AC5):
//! - Only `valid` bids at or above the reserve take part. Bids below the
//!   reserve are returned as losers with a full refund.
//! - Sort by amount descending, then earlier block height, then lower leaf hash.
//! - Uniform price: the top N bids win and each pays the N-th highest bid. If
//!   fewer than N bids take part, every bidder wins and pays the lowest bid.
//! - First price: the top N bids win and each pays its own bid.
//!
//! The result is independent of input order: the coordinator, the CLI and the
//! WASM auditor must produce byte-identical results.

use std::{cmp::Ordering, collections::BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Zatoshi;

/// Identifies the clearing algorithm in published results.
pub const ALGORITHM_VERSION: &str =
    concat!("sealforge-core/", env!("CARGO_PKG_VERSION"), "/clearing-v1");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuctionMode {
    UniformPrice,
    FirstPrice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionRules {
    pub mode: AuctionMode,
    pub quantity: u32,
    pub reserve: Zatoshi,
}

/// A valid bid as seen by the clearing engine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearingBid {
    #[serde(with = "hex::serde")]
    pub leaf_hash: [u8; 32],
    pub block_height: u32,
    pub amount: Zatoshi,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BidOutcome {
    #[serde(with = "hex::serde")]
    pub leaf_hash: [u8; 32],
    pub is_winner: bool,
    pub price_paid: Zatoshi,
    pub refund_due: Zatoshi,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearingResult {
    /// `None` when no bid met the reserve.
    pub clearing_price: Option<Zatoshi>,
    /// Sum of all bids that took part (at or above the reserve).
    pub total_volume: Zatoshi,
    /// Number of bids that took part.
    pub valid_bid_count: u32,
    pub winner_count: u32,
    /// One outcome per input bid, sorted by leaf hash.
    pub outcomes: Vec<BidOutcome>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClearingError {
    #[error("auction quantity must be at least 1")]
    ZeroQuantity,
    #[error("two bids share the leaf hash {0}")]
    DuplicateLeaf(String),
    #[error("total bid volume overflows the ZEC supply")]
    VolumeOverflow,
}

/// Canonical bid priority: amount descending, then block height ascending,
/// then leaf hash ascending.
fn priority(a: &ClearingBid, b: &ClearingBid) -> Ordering {
    b.amount
        .cmp(&a.amount)
        .then(a.block_height.cmp(&b.block_height))
        .then(a.leaf_hash.cmp(&b.leaf_hash))
}

/// Compute the clearing result. Deterministic for any input order.
pub fn clear(rules: &AuctionRules, bids: &[ClearingBid]) -> Result<ClearingResult, ClearingError> {
    if rules.quantity == 0 {
        return Err(ClearingError::ZeroQuantity);
    }

    let mut by_leaf: Vec<&ClearingBid> = bids.iter().collect();
    by_leaf.sort_by_key(|b| b.leaf_hash);
    if let Some(pair) = by_leaf
        .windows(2)
        .find(|w| w[0].leaf_hash == w[1].leaf_hash)
    {
        return Err(ClearingError::DuplicateLeaf(hex::encode(pair[0].leaf_hash)));
    }

    let mut eligible: Vec<&ClearingBid> =
        bids.iter().filter(|b| b.amount >= rules.reserve).collect();
    eligible.sort_by(|a, b| priority(a, b));

    let total_volume = eligible
        .iter()
        .try_fold(Zatoshi::ZERO, |acc, b| acc.checked_add(b.amount))
        .ok_or(ClearingError::VolumeOverflow)?;

    let winner_count = eligible.len().min(rules.quantity as usize);
    let winners: BTreeSet<[u8; 32]> = eligible[..winner_count]
        .iter()
        .map(|b| b.leaf_hash)
        .collect();
    let clearing_price = eligible[..winner_count].last().map(|b| b.amount);

    let outcomes = by_leaf
        .iter()
        .map(|bid| {
            let is_winner = winners.contains(&bid.leaf_hash);
            let price_paid = match (is_winner, rules.mode) {
                (false, _) => Zatoshi::ZERO,
                (true, AuctionMode::FirstPrice) => bid.amount,
                (true, AuctionMode::UniformPrice) => {
                    clearing_price.expect("winners imply a clearing price")
                }
            };
            BidOutcome {
                leaf_hash: bid.leaf_hash,
                is_winner,
                price_paid,
                refund_due: bid
                    .amount
                    .checked_sub(price_paid)
                    .expect("a winner never pays more than its bid"),
            }
        })
        .collect();

    Ok(ClearingResult {
        clearing_price,
        total_volume,
        valid_bid_count: eligible.len() as u32,
        winner_count: winner_count as u32,
        outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bid(id: u8, height: u32, amount: u64) -> ClearingBid {
        ClearingBid {
            leaf_hash: [id; 32],
            block_height: height,
            amount: Zatoshi(amount),
        }
    }

    fn rules(mode: AuctionMode, quantity: u32, reserve: u64) -> AuctionRules {
        AuctionRules {
            mode,
            quantity,
            reserve: Zatoshi(reserve),
        }
    }

    fn outcome(r: &ClearingResult, id: u8) -> &BidOutcome {
        r.outcomes.iter().find(|o| o.leaf_hash == [id; 32]).unwrap()
    }

    #[test]
    fn uniform_price_charges_nth_highest_bid() {
        let bids = [
            bid(1, 10, 500),
            bid(2, 10, 300),
            bid(3, 10, 400),
            bid(4, 10, 100),
        ];
        let r = clear(&rules(AuctionMode::UniformPrice, 2, 0), &bids).unwrap();
        assert_eq!(r.clearing_price, Some(Zatoshi(400)));
        assert_eq!(r.winner_count, 2);
        assert_eq!(r.total_volume, Zatoshi(1300));
        assert_eq!(outcome(&r, 1).price_paid, Zatoshi(400));
        assert_eq!(outcome(&r, 1).refund_due, Zatoshi(100));
        assert_eq!(outcome(&r, 3).refund_due, Zatoshi(0));
        assert!(!outcome(&r, 2).is_winner);
        assert_eq!(outcome(&r, 2).refund_due, Zatoshi(300));
    }

    #[test]
    fn first_price_charges_own_bid() {
        let bids = [bid(1, 10, 500), bid(2, 10, 300), bid(3, 10, 400)];
        let r = clear(&rules(AuctionMode::FirstPrice, 2, 0), &bids).unwrap();
        assert_eq!(outcome(&r, 1).price_paid, Zatoshi(500));
        assert_eq!(outcome(&r, 3).price_paid, Zatoshi(400));
        assert_eq!(outcome(&r, 1).refund_due, Zatoshi(0));
    }

    #[test]
    fn reserve_excludes_low_bids_with_full_refund() {
        let bids = [bid(1, 10, 500), bid(2, 10, 50)];
        let r = clear(&rules(AuctionMode::UniformPrice, 5, 100), &bids).unwrap();
        assert_eq!(r.valid_bid_count, 1);
        assert_eq!(r.total_volume, Zatoshi(500));
        assert!(!outcome(&r, 2).is_winner);
        assert_eq!(outcome(&r, 2).refund_due, Zatoshi(50));
    }

    #[test]
    fn undersubscribed_uniform_auction_charges_lowest_bid() {
        let bids = [bid(1, 10, 500), bid(2, 10, 300)];
        let r = clear(&rules(AuctionMode::UniformPrice, 10, 0), &bids).unwrap();
        assert_eq!(r.winner_count, 2);
        assert_eq!(r.clearing_price, Some(Zatoshi(300)));
    }

    #[test]
    fn ties_break_on_height_then_leaf_hash() {
        // Same amount: the earlier block wins. Same block: the lower leaf hash wins.
        let bids = [bid(9, 20, 100), bid(8, 10, 100), bid(7, 20, 100)];
        let r = clear(&rules(AuctionMode::UniformPrice, 2, 0), &bids).unwrap();
        assert!(outcome(&r, 8).is_winner);
        assert!(outcome(&r, 7).is_winner);
        assert!(!outcome(&r, 9).is_winner);
    }

    #[test]
    fn no_eligible_bids() {
        let r = clear(&rules(AuctionMode::UniformPrice, 1, 1000), &[bid(1, 1, 10)]).unwrap();
        assert_eq!(r.clearing_price, None);
        assert_eq!(r.winner_count, 0);
        assert_eq!(r.total_volume, Zatoshi::ZERO);
    }

    #[test]
    fn result_is_independent_of_input_order() {
        let mut bids: Vec<ClearingBid> = (0..=200u8)
            .map(|i| bid(i, u32::from(i % 7), u64::from(i % 13) * 1_000))
            .collect();
        let rules = rules(AuctionMode::UniformPrice, 17, 2_000);
        let a = clear(&rules, &bids).unwrap();
        bids.reverse();
        let b = clear(&rules, &bids).unwrap();
        bids.rotate_left(91);
        let c = clear(&rules, &bids).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(
            clear(&rules(AuctionMode::UniformPrice, 0, 0), &[]),
            Err(ClearingError::ZeroQuantity)
        );
        assert!(matches!(
            clear(
                &rules(AuctionMode::UniformPrice, 1, 0),
                &[bid(1, 1, 5), bid(1, 2, 6)]
            ),
            Err(ClearingError::DuplicateLeaf(_))
        ));
    }

    #[test]
    fn clears_twenty_thousand_bids_quickly() {
        let bids: Vec<ClearingBid> = (0..20_000u32)
            .map(|i| {
                let mut leaf = [0u8; 32];
                leaf[..4].copy_from_slice(&i.to_be_bytes());
                ClearingBid {
                    leaf_hash: leaf,
                    block_height: i % 500,
                    amount: Zatoshi(u64::from(i.wrapping_mul(2_654_435_761) % 1_000_000)),
                }
            })
            .collect();
        let start = std::time::Instant::now();
        let r = clear(&rules(AuctionMode::UniformPrice, 8_000, 0), &bids).unwrap();
        assert_eq!(r.winner_count, 8_000);
        // PRD/TRD target: < 1 s for 20k bids (generous bound for debug builds).
        assert!(start.elapsed().as_secs() < 5);
    }
}
