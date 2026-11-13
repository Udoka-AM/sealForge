//! Test vectors for every sealForge hash and wire format.
//!
//! Shared by `examples/vectors.rs` (prints them) and `tests/vectors.rs`
//! (checks them against the checked-in `tests/fixtures/vectors.json`).
//! `packages/bid` (TypeScript) checks the commitment, memo and URI fields of
//! the same file.
//!
//! The Merkle node and root are also recomputed here directly from the
//! personalizations and byte layouts in the docs, independently of
//! `merkle::MerkleTree`, and the two must agree.

use std::path::PathBuf;

use sealforge_core::{
    clearing::AuctionRules,
    memo::{BidMemo, RevealMemo},
    merkle::{leaf_hash, MerkleTree},
    receipt::{commitment, price_commitment},
    result::{compute_result, ScannedBid},
    zip321::payment_uri,
    Zatoshi,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub const CODE: &str = "7K3QX9MZ2P";
pub const REFUND_UA: &str = "utest1refundexample";
pub const ESCROW_UA: &str = "utest1escrowexample";
pub const SALT: [u8; 32] = [1; 32];
pub const AMOUNT_ZAT: u64 = 42_000_000;
pub const PRICE_ZAT: u64 = 42_000_000;
pub const DEPOSIT_ZAT: u64 = 100_000_000;

fn blake2b(personal: &[u8; 16], parts: &[&[u8]]) -> [u8; 32] {
    let mut state = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(personal)
        .to_state();
    for p in parts {
        state.update(p);
    }
    state.finalize().as_bytes().try_into().unwrap()
}

#[derive(Deserialize)]
struct Fixture {
    rules: AuctionRules,
    bids: Vec<ScannedBid>,
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn vectors() -> Value {
    // Deposit mode.
    let bid_commitment = commitment(CODE, REFUND_UA, &SALT);
    let bid_memo = BidMemo {
        auction_code: CODE.into(),
        commitment: bid_commitment,
        refund_ua: REFUND_UA.into(),
    }
    .encode()
    .unwrap();
    let uri = payment_uri(ESCROW_UA, Zatoshi(AMOUNT_ZAT), &bid_memo);

    // Sealed-reveal mode.
    let price_c = price_commitment(CODE, REFUND_UA, PRICE_ZAT, &SALT);
    let sealed_memo = BidMemo {
        auction_code: CODE.into(),
        commitment: price_c,
        refund_ua: REFUND_UA.into(),
    }
    .encode()
    .unwrap();
    let reveal_memo = RevealMemo {
        auction_code: CODE.into(),
        commitment: price_c,
        price_zat: PRICE_ZAT,
        salt: SALT,
    }
    .encode()
    .unwrap();

    // Merkle: two leaves, so the tree is one node and no padding.
    let txid_a = [0x11u8; 32];
    let txid_b = [0x22u8; 32];
    let leaf_a = leaf_hash(&txid_a, 1, AMOUNT_ZAT, &bid_commitment);
    let leaf_b = leaf_hash(&txid_b, 0, DEPOSIT_ZAT, &price_c);
    // Independent recomputation from the documented layout.
    let leaf_a_manual = blake2b(
        b"sealforge_leaf1\0",
        &[
            &txid_a,
            &1u32.to_le_bytes(),
            &AMOUNT_ZAT.to_le_bytes(),
            &bid_commitment,
        ],
    );
    assert_eq!(leaf_a, leaf_a_manual, "leaf layout");
    let (lo, hi) = if leaf_a < leaf_b {
        (leaf_a, leaf_b)
    } else {
        (leaf_b, leaf_a)
    };
    let node = blake2b(b"sealforge_node1\0", &[&lo, &hi]);
    let root_2 = blake2b(b"sealforge_root1\0", &[&2u64.to_le_bytes(), &node]);
    assert_eq!(
        MerkleTree::from_leaves(vec![leaf_a, leaf_b])
            .unwrap()
            .root(),
        root_2,
        "node and root layout"
    );
    // Three leaves: width 4, the fourth leaf is 32 zero bytes.
    let leaf_c = leaf_hash(&[0x33; 32], 2, 7_000_000, &[0x44; 32]);
    let root_3 = MerkleTree::from_leaves(vec![leaf_a, leaf_b, leaf_c])
        .unwrap()
        .root();

    // The 20-bid uniform-price fixture from tests/pipeline.rs.
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(fixtures_dir().join("bids-20.json")).unwrap(),
    )
    .unwrap();
    let result_20 = compute_result(&fixture.rules, &fixture.bids).unwrap();

    json!({
        "inputs": {
            "auction_code": CODE,
            "refund_ua": REFUND_UA,
            "escrow_ua": ESCROW_UA,
            "salt": hex::encode(SALT),
            "amount_zat": AMOUNT_ZAT,
            "price_zat": PRICE_ZAT,
            "deposit_zat": DEPOSIT_ZAT,
        },
        "deposit_mode": {
            "bid_commitment": hex::encode(bid_commitment),
            "bid_memo": bid_memo,
            "payment_uri": uri,
        },
        "sealed_reveal_mode": {
            "price_commitment": hex::encode(price_c),
            "bid_memo": sealed_memo,
            "reveal_memo": reveal_memo,
        },
        "merkle": {
            "leaf_a": {
                "txid": hex::encode(txid_a), "output_index": 1,
                "amount_zat": AMOUNT_ZAT, "commitment": hex::encode(bid_commitment),
                "leaf": hex::encode(leaf_a),
            },
            "leaf_b": {
                "txid": hex::encode(txid_b), "output_index": 0,
                "amount_zat": DEPOSIT_ZAT, "commitment": hex::encode(price_c),
                "leaf": hex::encode(leaf_b),
            },
            "node_ab": hex::encode(node),
            "root_2_leaves": hex::encode(root_2),
            "leaf_c": {
                "txid": hex::encode([0x33u8; 32]), "output_index": 2,
                "amount_zat": 7_000_000, "commitment": hex::encode([0x44u8; 32]),
                "leaf": hex::encode(leaf_c),
            },
            "root_3_leaves": hex::encode(root_3),
        },
        "result_20_bids": {
            "fixture": "crates/sealforge-core/tests/fixtures/bids-20.json",
            "mode": "uniform_price",
            "quantity": fixture.rules.quantity,
            "reserve_zat": fixture.rules.reserve.0,
            "clearing_price_zat": result_20.clearing.clearing_price.map(|z| z.0),
            "total_volume_zat": result_20.clearing.total_volume.0,
            "valid_bid_count": result_20.clearing.valid_bid_count,
            "winner_count": result_20.clearing.winner_count,
            "merkle_root": hex::encode(result_20.merkle_root),
        },
    })
}
