//! Prints the cross-language test vectors (JSON). The same values are pinned in
//! `tests/fixtures/vectors.json`, checked by `tests/vectors.rs` (Rust) and
//! `packages/bid/src/bid.test.ts` (TypeScript).
//!
//! `cargo run -p sealforge-core --example vectors`

#[path = "../tests/support/vectors.rs"]
mod support;

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&support::vectors()).unwrap()
    );
}
