//! Refund planner (Milestone 3, task 3.8). ZIP 318-style refund privacy.
//!
//! - Each refund is split into canonical notes ({1, 2, 5} × 10^k zat) only.
//!   Any remainder below the smallest denomination (0.0001 ZEC) is **dust**
//!   and is not sent: it is worth less than the network fee for one note, and
//!   a non-canonical note would have a unique value that links the refund to
//!   the bid (measured by tools/linkability-harness: 98% linked when the
//!   remainder joined the smallest note, 0.1% when dust is dropped).
//!   Bids made in the web app use at most 4 decimals, so they have no dust.
//! - Each note gets its own random delay in `[min_delay, max_delay]`.
//! - Due notes are grouped into batches that interleave bidders, so a single
//!   transaction rarely carries one bidder's whole refund.
//!
//! This module stays pure: randomness is injected by the caller (the
//! coordinator uses a CSPRNG), which keeps the logic testable and lets the
//! splitting rule be checked publicly.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{denominations, Zatoshi};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundPolicy {
    /// Shortest delay before a note may be sent, in seconds.
    pub min_delay_secs: u64,
    /// Longest delay before a note may be sent, in seconds.
    pub max_delay_secs: u64,
    /// Maximum outputs per escrow transaction.
    pub max_outputs_per_batch: usize,
    /// Due notes are held until this many bidders' notes can share a batch...
    pub min_owners_per_batch: usize,
    /// ...or the oldest due note has waited this long, in seconds.
    pub max_hold_secs: u64,
}

impl Default for RefundPolicy {
    /// PRD AC9 default: delays of 10–240 minutes.
    fn default() -> Self {
        Self {
            min_delay_secs: 10 * 60,
            max_delay_secs: 240 * 60,
            max_outputs_per_batch: 20,
            min_owners_per_batch: 3,
            max_hold_secs: 30 * 60,
        }
    }
}

/// One planned refund output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedNote {
    pub value: Zatoshi,
    /// Seconds after the plan time before this note may be sent.
    pub delay_secs: u64,
}

/// Split a refund into canonical notes, largest first. The sub-denomination
/// dust (see [`refund_dust`]) is not included.
pub fn split_refund(due: Zatoshi) -> Vec<Zatoshi> {
    denominations::split(due).0
}

/// The part of a refund below the smallest denomination, which is not sent.
pub fn refund_dust(due: Zatoshi) -> Zatoshi {
    Zatoshi(due.0 % denominations::MIN_DENOMINATION)
}

/// Plan the notes for one refund. `random` must return uniformly random `u64`s
/// from a CSPRNG.
pub fn plan_refund(
    due: Zatoshi,
    policy: &RefundPolicy,
    random: &mut impl FnMut() -> u64,
) -> Vec<PlannedNote> {
    assert!(
        policy.min_delay_secs <= policy.max_delay_secs,
        "min delay must not exceed max delay"
    );
    let span = policy.max_delay_secs - policy.min_delay_secs + 1;
    split_refund(due)
        .into_iter()
        .map(|value| PlannedNote {
            value,
            // Modulo bias is negligible for spans of hours against a u64.
            delay_secs: policy.min_delay_secs + random() % span,
        })
        .collect()
}

/// A note that is due to be sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DueNote<Id> {
    pub id: Id,
    /// Identifies whose refund this note belongs to (e.g. the bid id).
    pub owner: Id,
    pub value: Zatoshi,
}

/// Group due notes into batches of at most `max_outputs`, interleaving owners
/// round-robin so each batch mixes as many bidders as possible.
///
/// The caller shuffles `notes` with a CSPRNG first. Owner order follows the
/// first appearance in that shuffled input, so the output is deterministic for
/// a given input.
pub fn batch_due_notes<Id: Clone + Ord>(
    notes: &[DueNote<Id>],
    max_outputs: usize,
) -> Vec<Vec<DueNote<Id>>> {
    assert!(max_outputs > 0, "max_outputs must be at least 1");

    let mut owner_order: Vec<Id> = Vec::new();
    let mut queues: BTreeMap<Id, Vec<&DueNote<Id>>> = BTreeMap::new();
    for note in notes {
        let queue = queues.entry(note.owner.clone()).or_default();
        if queue.is_empty() {
            owner_order.push(note.owner.clone());
        }
        queue.push(note);
    }
    for queue in queues.values_mut() {
        queue.reverse(); // pop() takes notes in input order
    }

    let mut interleaved = Vec::with_capacity(notes.len());
    while interleaved.len() < notes.len() {
        for owner in &owner_order {
            if let Some(note) = queues.get_mut(owner).and_then(Vec::pop) {
                interleaved.push(note.clone());
            }
        }
    }
    interleaved.chunks(max_outputs).map(<[_]>::to_vec).collect()
}

/// Whether an auction's due notes should be batched now, or held so more
/// bidders' notes can share the transaction (task 3.9: batches mix at least
/// `min_owners_per_batch` bidders when that many are available).
///
/// Batch when enough distinct owners are due, when the oldest due note has
/// waited `max_hold_secs`, or when no planned note is still to come.
pub fn ready_to_batch(
    due_owners: usize,
    oldest_due_age_secs: u64,
    notes_not_yet_due: usize,
    policy: &RefundPolicy,
) -> bool {
    due_owners > 0
        && (due_owners >= policy.min_owners_per_batch
            || oldest_due_age_secs >= policy.max_hold_secs
            || notes_not_yet_due == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::denominations::{is_canonical, MIN_DENOMINATION};

    /// Deterministic test RNG (splitmix64). Production code uses a CSPRNG.
    fn rng(mut state: u64) -> impl FnMut() -> u64 {
        move || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
    }

    #[test]
    fn notes_are_canonical_and_sum_to_due_minus_dust() {
        for due in [
            0,
            1,
            9_999,
            10_000,
            10_001,
            37_000_000,
            123_456_789,
            98_765_432_101,
        ] {
            let notes = split_refund(Zatoshi(due));
            let dust = refund_dust(Zatoshi(due));
            assert!(notes.iter().all(|n| is_canonical(*n)), "due={due}");
            assert!(dust.0 < MIN_DENOMINATION);
            assert_eq!(
                notes.iter().map(|n| n.0).sum::<u64>() + dust.0,
                due,
                "due={due}"
            );
        }
    }

    #[test]
    fn dust_is_not_sent() {
        // 0.37000123 ZEC = 0.2 + 0.1 + 0.05 + 0.02, plus 123 zat of dust.
        let due = Zatoshi(37_000_123);
        assert_eq!(
            split_refund(due),
            vec![
                Zatoshi(20_000_000),
                Zatoshi(10_000_000),
                Zatoshi(5_000_000),
                Zatoshi(2_000_000)
            ]
        );
        assert_eq!(refund_dust(due), Zatoshi(123));
        assert!(split_refund(Zatoshi(MIN_DENOMINATION - 1)).is_empty());
        assert!(split_refund(Zatoshi::ZERO).is_empty());
    }

    #[test]
    fn delays_stay_within_policy() {
        let policy = RefundPolicy::default();
        let mut random = rng(7);
        for _ in 0..200 {
            for note in plan_refund(Zatoshi(987_654_321), &policy, &mut random) {
                assert!((policy.min_delay_secs..=policy.max_delay_secs).contains(&note.delay_secs));
            }
        }
    }

    #[test]
    fn batches_interleave_bidders() {
        let notes: Vec<DueNote<u32>> = (0..3u32)
            .flat_map(|owner| {
                (0..4u32).map(move |i| DueNote {
                    id: owner * 10 + i,
                    owner,
                    value: Zatoshi(10_000),
                })
            })
            .collect();
        let batches = batch_due_notes(&notes, 3);
        assert_eq!(batches.len(), 4);
        for batch in &batches {
            let mut owners: Vec<u32> = batch.iter().map(|n| n.owner).collect();
            owners.dedup();
            assert_eq!(owners.len(), 3, "each batch mixes all three bidders");
        }
        let total: usize = batches.iter().map(Vec::len).sum();
        assert_eq!(total, notes.len());
    }

    #[test]
    fn batching_respects_max_outputs_and_keeps_every_note() {
        let notes: Vec<DueNote<u32>> = (0..53u32)
            .map(|i| DueNote {
                id: i,
                owner: i % 5,
                value: Zatoshi(20_000),
            })
            .collect();
        let batches = batch_due_notes(&notes, 20);
        assert!(batches.iter().all(|b| b.len() <= 20));
        let mut ids: Vec<u32> = batches.iter().flatten().map(|n| n.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, (0..53).collect::<Vec<_>>());
    }

    #[test]
    fn batches_wait_for_three_bidders_or_the_hold() {
        let p = RefundPolicy::default();
        assert!(!ready_to_batch(0, 10_000, 0, &p), "nothing due");
        assert!(!ready_to_batch(1, 60, 5, &p), "hold for more bidders");
        assert!(!ready_to_batch(2, 60, 5, &p));
        assert!(ready_to_batch(3, 0, 5, &p), "three bidders mix");
        assert!(ready_to_batch(1, p.max_hold_secs, 5, &p), "hold expired");
        assert!(ready_to_batch(1, 0, 0, &p), "nothing else is coming");
    }
}
