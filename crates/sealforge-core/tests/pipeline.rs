//! End-to-end checks over fixture bid sets: clearing → Merkle → refunds.
//!
//! Golden files pin the exact published output so the coordinator, CLI and
//! WASM auditor can be checked against the same vectors. Regenerate with
//! `UPDATE_FIXTURES=1 cargo test -p sealforge-core --test pipeline` and review the diff.

use std::path::PathBuf;

use sealforge_core::{
    clearing::{AuctionMode, AuctionRules},
    merkle::{verify_proof, MerkleTree},
    refunds::split_refund,
    result::{compute_result, AuctionResult, ScannedBid},
    Zatoshi,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Fixture {
    rules: AuctionRules,
    bids: Vec<ScannedBid>,
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Deterministic pseudo-random bid set (splitmix64).
fn generate(count: u32, seed: u64) -> Vec<ScannedBid> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    (0..count)
        .map(|i| {
            let mut txid = [0u8; 32];
            let mut commitment = [0u8; 32];
            for chunk in txid.chunks_mut(8).chain(commitment.chunks_mut(8)) {
                chunk.copy_from_slice(&next().to_le_bytes());
            }
            ScannedBid {
                txid,
                output_index: i % 3,
                block_height: 3_400_000 + (next() % 500) as u32,
                // 0.001 – 5 ZEC
                amount: Zatoshi(100_000 + next() % 500_000_000),
                commitment,
            }
        })
        .collect()
}

fn check(name: &str, rules: AuctionRules, count: u32, seed: u64) {
    let input = fixture_path(&format!("{name}.json"));
    let expected = fixture_path(&format!("{name}.expected.json"));
    let update = std::env::var_os("UPDATE_FIXTURES").is_some();

    let fixture: Fixture = if update || !input.exists() {
        let f = Fixture {
            rules,
            bids: generate(count, seed),
        };
        std::fs::write(&input, serde_json::to_string_pretty(&f).unwrap()).unwrap();
        f
    } else {
        serde_json::from_str(&std::fs::read_to_string(&input).unwrap()).unwrap()
    };

    let result = compute_result(&fixture.rules, &fixture.bids).unwrap();

    // Invariants: refunds plus payments equal deposits, and every refund splits exactly.
    let paid: u64 = result
        .clearing
        .outcomes
        .iter()
        .map(|o| o.price_paid.0)
        .sum();
    let refunded: u64 = result
        .clearing
        .outcomes
        .iter()
        .map(|o| o.refund_due.0)
        .sum();
    let deposited: u64 = fixture.bids.iter().map(|b| b.amount.0).sum();
    assert_eq!(paid + refunded, deposited);
    for o in &result.clearing.outcomes {
        assert_eq!(
            split_refund(o.refund_due).iter().map(|n| n.0).sum::<u64>(),
            o.refund_due.0
        );
    }

    // Every bidder gets a valid inclusion proof.
    let leaves: Vec<_> = fixture.bids.iter().map(ScannedBid::leaf_hash).collect();
    let tree = MerkleTree::from_leaves(leaves.clone()).unwrap();
    assert_eq!(tree.root(), result.merkle_root);
    for leaf in &leaves {
        assert!(verify_proof(
            &result.merkle_root,
            leaf,
            &tree.proof(leaf).unwrap()
        ));
    }

    if update || !expected.exists() {
        std::fs::write(&expected, serde_json::to_string_pretty(&result).unwrap()).unwrap();
    } else {
        let golden: AuctionResult =
            serde_json::from_str(&std::fs::read_to_string(&expected).unwrap()).unwrap();
        assert_eq!(result.clearing, golden.clearing, "{name}: clearing changed");
        assert_eq!(
            result.merkle_root, golden.merkle_root,
            "{name}: Merkle root changed"
        );
    }
}

#[test]
fn bids_20_uniform() {
    let rules = AuctionRules {
        mode: AuctionMode::UniformPrice,
        quantity: 10,
        reserve: Zatoshi(1_000_000),
    };
    check("bids-20", rules, 20, 1);
}

#[test]
fn bids_500_first_price() {
    let rules = AuctionRules {
        mode: AuctionMode::FirstPrice,
        quantity: 100,
        reserve: Zatoshi(10_000_000),
    };
    check("bids-500", rules, 500, 2);
}
