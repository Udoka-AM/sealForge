//! Linkability harness (task 4.3, PRD AC13).
//!
//! **Threat modelled.** After close, the published viewing key reveals every
//! bid's amount, so anyone can compute each bid's refund. The leak we measure
//! is at the *exit*: when a bidder unshields or swaps refund money on a public
//! chain, that amount is visible. An observer who sees every exit event tries
//! to attribute each one to a bid.
//!
//! We assume the worst case for the bidder: every refund note is exited
//! separately and immediately on arrival. A bid counts as **linked** when at
//! least one of its refund notes has a value no other refund note in the
//! auction has, so the observer can attribute that exit with certainty.
//! Timing is left out of the metric because sealForge gives every note an
//! independent random delay, so arrival times carry no per-bid signal; in the
//! naive flow timing adds nothing either, since amounts already link ~100%.

use std::collections::HashMap;

use rand::{rngs::StdRng, Rng, SeedableRng};
use sealforge_core::{
    amount::COIN,
    clearing::{clear, AuctionMode, AuctionRules, ClearingBid},
    denominations::canonical_denominations,
    refunds::split_refund,
    Zatoshi,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    /// One refund of the exact amount due per bidder.
    Naive,
    /// Ablation: the earlier design, where sub-0.0001 ZEC remainders joined the
    /// smallest note. Kept to show why sealForge drops dust instead.
    RemainderInSmallestNote,
    /// sealForge refund notes (canonical only, dust not sent), arbitrary bid amounts.
    SealforgeRawBids,
    /// sealForge refund notes, bidders accept the UI's suggestion to round to a common amount.
    SealforgeRoundedBids,
}

impl Scenario {
    pub const ALL: [Scenario; 4] = [
        Self::Naive,
        Self::RemainderInSmallestNote,
        Self::SealforgeRawBids,
        Self::SealforgeRoundedBids,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Naive => "Naive refunds",
            Self::RemainderInSmallestNote => "Remainder in smallest note",
            Self::SealforgeRawBids => "sealForge, any bid amount",
            Self::SealforgeRoundedBids => "sealForge, rounded bids",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Params {
    pub bidders: usize,
    pub quantity: u32,
    /// Median bid in ZEC (bids are log-normally distributed around it).
    pub median_bid_zec: f64,
    /// Log-normal spread (sigma).
    pub spread: f64,
    pub trials: u64,
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            bidders: 500,
            quantity: 50,
            median_bid_zec: 0.5,
            spread: 1.0,
            trials: 20,
            seed: 7,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario: Scenario,
    pub label: String,
    /// Mean share of refunded bids the observer can link, 0.0–1.0.
    pub linked_share: f64,
    /// Mean refund notes per refunded bid.
    pub notes_per_refund: f64,
}

/// Standard normal sample (Box–Muller).
fn normal(rng: &mut StdRng) -> f64 {
    let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
    let u2: f64 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Nearest canonical denomination (what the bid page suggests).
fn round_canonical(zat: u64) -> u64 {
    canonical_denominations()
        .into_iter()
        .min_by_key(|d| d.abs_diff(zat))
        .expect("denominations exist")
}

fn bids(params: &Params, rounded: bool, rng: &mut StdRng) -> Vec<u64> {
    let (min, max) = (COIN / 100, 100 * COIN);
    (0..params.bidders)
        .map(|_| {
            let zec = params.median_bid_zec * (params.spread * normal(rng)).exp();
            let zat = ((zec * COIN as f64) as u64).clamp(min, max);
            if rounded {
                round_canonical(zat)
            } else {
                zat
            }
        })
        .collect()
}

/// Refund owed to each bidder after a uniform-price clearing.
fn refunds(params: &Params, amounts: &[u64]) -> Vec<u64> {
    let clearing_bids: Vec<ClearingBid> = amounts
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            let mut leaf = [0u8; 32];
            leaf[..8].copy_from_slice(&(i as u64).to_be_bytes());
            ClearingBid {
                leaf_hash: leaf,
                block_height: 1,
                amount: Zatoshi(a),
            }
        })
        .collect();
    let rules = AuctionRules {
        mode: AuctionMode::UniformPrice,
        quantity: params.quantity,
        reserve: Zatoshi(COIN / 100),
    };
    let result = clear(&rules, &clearing_bids).expect("valid auction");
    let by_leaf: HashMap<[u8; 32], u64> = result
        .outcomes
        .iter()
        .map(|o| (o.leaf_hash, o.refund_due.0))
        .collect();
    clearing_bids
        .iter()
        .map(|b| by_leaf[&b.leaf_hash])
        .collect()
}

/// The exit events each bidder's refund produces.
pub fn notes_for(scenario: Scenario, refund: u64) -> Vec<u64> {
    match scenario {
        Scenario::Naive => (refund > 0).then_some(refund).into_iter().collect(),
        Scenario::RemainderInSmallestNote => {
            let (mut notes, remainder) = sealforge_core::denominations::split(Zatoshi(refund));
            match (notes.last_mut(), remainder) {
                (Some(smallest), Some(rem)) => smallest.0 += rem.0,
                (None, Some(rem)) => notes.push(rem),
                _ => {}
            }
            notes.into_iter().map(|z| z.0).collect()
        }
        _ => split_refund(Zatoshi(refund))
            .into_iter()
            .map(|z| z.0)
            .collect(),
    }
}

/// Share of refunded bids with at least one uniquely valued note.
pub fn linked_share(notes_per_bid: &[Vec<u64>]) -> (f64, f64) {
    let mut counts: HashMap<u64, usize> = HashMap::new();
    for notes in notes_per_bid {
        for &n in notes {
            *counts.entry(n).or_default() += 1;
        }
    }
    let refunded: Vec<&Vec<u64>> = notes_per_bid.iter().filter(|n| !n.is_empty()).collect();
    if refunded.is_empty() {
        return (0.0, 0.0);
    }
    let linked = refunded
        .iter()
        .filter(|notes| notes.iter().any(|n| counts[n] == 1))
        .count();
    let notes: usize = refunded.iter().map(|n| n.len()).sum();
    (
        linked as f64 / refunded.len() as f64,
        notes as f64 / refunded.len() as f64,
    )
}

/// Run every scenario over `params.trials` seeded auctions.
pub fn run(params: &Params) -> Vec<ScenarioResult> {
    Scenario::ALL
        .into_iter()
        .map(|scenario| {
            let (mut linked, mut per) = (0.0, 0.0);
            for trial in 0..params.trials {
                let mut rng = StdRng::seed_from_u64(params.seed.wrapping_add(trial));
                let amounts = bids(params, scenario == Scenario::SealforgeRoundedBids, &mut rng);
                let notes: Vec<Vec<u64>> = refunds(params, &amounts)
                    .into_iter()
                    .map(|r| notes_for(scenario, r))
                    .collect();
                let (l, p) = linked_share(&notes);
                linked += l;
                per += p;
            }
            let trials = params.trials.max(1) as f64;
            ScenarioResult {
                scenario,
                label: scenario.label().to_owned(),
                linked_share: linked / trials,
                notes_per_refund: per / trials,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_values_link_and_shared_values_do_not() {
        // Bid A's 0.37 note is unique; B and C share identical notes.
        let notes = vec![
            vec![37_000_000],
            vec![20_000_000, 5_000_000],
            vec![20_000_000, 5_000_000],
        ];
        let (share, _) = linked_share(&notes);
        assert!((share - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn winners_with_nothing_to_refund_are_not_counted() {
        let (share, per) = linked_share(&[vec![], vec![10_000], vec![10_000]]);
        assert_eq!(share, 0.0);
        assert_eq!(per, 1.0);
    }

    #[test]
    fn sealforge_meets_the_ac13_target_and_naive_does_not() {
        let results = run(&Params {
            trials: 3,
            ..Params::default()
        });
        let share = |s: Scenario| {
            results
                .iter()
                .find(|r| r.scenario == s)
                .unwrap()
                .linked_share
        };
        assert!(share(Scenario::Naive) > 0.95);
        assert!(
            share(Scenario::RemainderInSmallestNote) > 0.8,
            "unique remainders link"
        );
        // PRD AC13 target: under 5% linkable.
        assert!(share(Scenario::SealforgeRawBids) < 0.05);
        assert!(share(Scenario::SealforgeRoundedBids) < 0.05);
    }

    #[test]
    fn results_are_reproducible_for_a_seed() {
        let p = Params {
            trials: 2,
            ..Params::default()
        };
        let a: Vec<f64> = run(&p).iter().map(|r| r.linked_share).collect();
        let b: Vec<f64> = run(&p).iter().map(|r| r.linked_share).collect();
        assert_eq!(a, b);
    }
}
