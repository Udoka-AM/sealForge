//! Prints the cross-language test vectors used by packages/bid (TypeScript).
//! `cargo run -p sealforge-core --example vectors`

use sealforge_core::{memo::BidMemo, receipt::commitment, zip321::payment_uri, Zatoshi};

fn main() {
    let code = "7K3QX9MZ2P";
    let refund = "utest1refundexample";
    let salt = [1u8; 32];
    let c = commitment(code, refund, &salt);
    let memo = BidMemo {
        auction_code: code.into(),
        commitment: c,
        refund_ua: refund.into(),
    }
    .encode()
    .unwrap();
    println!("commitment = {}", hex::encode(c));
    println!("memo = {memo}");
    println!(
        "uri = {}",
        payment_uri("utest1escrowexample", Zatoshi(42_000_000), &memo)
    );
}
