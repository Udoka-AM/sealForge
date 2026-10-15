//! Close-pipeline timing for docs/evidence/perf.md (task 5.5):
//! `cargo run --release -p sealforge-core --example bench_close`
//!
//! 20,000 bids through selection (memo checks, window, reserve, duplicates),
//! clearing and the Merkle tree, as the coordinator and auditor run them.

use std::time::Instant;

use sealforge_core::{
    clearing::{AuctionMode, AuctionRules},
    memo::BidMemo,
    result::compute_result,
    selection::{included_bids, BidWindow, Payment},
    Zatoshi,
};

fn main() {
    const N: u32 = 20_000;
    let code = "BENCH00001";
    let payments: Vec<Payment> = (0..N)
        .map(|i| {
            let mut txid = [0u8; 32];
            txid[..4].copy_from_slice(&i.to_be_bytes());
            let mut commitment = [7u8; 32];
            commitment[..4].copy_from_slice(&i.to_le_bytes());
            Payment {
                txid,
                output_index: 0,
                block_height: 1_000 + i % 3_500,
                amount: Zatoshi(10_000 * u64::from(1 + i.wrapping_mul(2_654_435_761) % 10_000)),
                memo: Some(BidMemo {
                    auction_code: code.into(),
                    commitment,
                    refund_ua: "utest1refund".into(),
                }),
            }
        })
        .collect();
    let window = BidWindow {
        auction_code: code.into(),
        open_height: 1_000,
        close_height: 4_500,
        reserve: Zatoshi(10_000),
    };
    let rules = AuctionRules {
        mode: AuctionMode::UniformPrice,
        quantity: 1_000,
        reserve: Zatoshi(10_000),
    };

    let mut best = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let included = included_bids(&window, &payments);
        let result = compute_result(&rules, &included).unwrap();
        let secs = start.elapsed().as_secs_f64();
        assert_eq!(result.clearing.winner_count, 1_000);
        best = best.min(secs);
    }
    println!(
        "{N} bids: selection + clearing + Merkle root in {:.1} ms (best of 5)",
        best * 1e3
    );
}
