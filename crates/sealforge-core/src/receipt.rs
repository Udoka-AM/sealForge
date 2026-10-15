//! Bidder receipts and bid commitments (Milestone 3, task 3.2).
//!
//! commitment = BLAKE2b-256(personal = "sealforge_bid_v1", auction_code ‖ refund_ua ‖ salt)
//! The salt is generated in the browser and never sent to the server.

use serde::{Deserialize, Serialize};

pub const COMMITMENT_PERSONAL: &[u8; 16] = b"sealforge_bid_v1";

/// Compute a bid commitment.
pub fn commitment(auction_code: &str, refund_ua: &str, salt: &[u8; 32]) -> [u8; 32] {
    let hash = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(COMMITMENT_PERSONAL)
        .to_state()
        .update(auction_code.as_bytes())
        .update(&[0])
        .update(refund_ua.as_bytes())
        .update(&[0])
        .update(salt)
        .finalize();
    hash.as_bytes().try_into().expect("32-byte hash")
}

/// Personalization for commit-reveal price commitments (PRD F13).
pub const PRICE_COMMITMENT_PERSONAL: &[u8; 16] = b"sealforge_rev_v1";

/// Commit-reveal mode: the deposit memo carries this instead of [`commitment`],
/// binding the hidden price as well.
///
/// commitment = BLAKE2b-256(personal = "sealforge_rev_v1",
///                          auction_code ‖ 0 ‖ refund_ua ‖ 0 ‖ price_zat (u64 LE) ‖ salt)
pub fn price_commitment(
    auction_code: &str,
    refund_ua: &str,
    price_zat: u64,
    salt: &[u8; 32],
) -> [u8; 32] {
    let hash = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(PRICE_COMMITMENT_PERSONAL)
        .to_state()
        .update(auction_code.as_bytes())
        .update(&[0])
        .update(refund_ua.as_bytes())
        .update(&[0])
        .update(&price_zat.to_le_bytes())
        .update(salt)
        .finalize();
    hash.as_bytes().try_into().expect("32-byte hash")
}

/// The receipt a bidder downloads (`sealforge-receipt-*.json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub version: u8,
    pub auction_code: String,
    pub amount_zat: u64,
    pub refund_ua: String,
    #[serde(with = "hex_32")]
    pub salt: [u8; 32],
    #[serde(with = "hex_32")]
    pub commitment: [u8; 32],
    /// Commit-reveal mode only: the hidden price. `amount_zat` is then the deposit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_zat: Option<u64>,
}

mod hex_32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        hex::decode(&s)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| serde::de::Error::custom("expected 32-byte hex"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared with packages/bid (TypeScript): both must produce this exact value.
    #[test]
    fn cross_language_vector() {
        assert_eq!(
            hex::encode(commitment("7K3QX9MZ2P", "utest1refundexample", &[1; 32])),
            "76d9fde6644588bafdcfda79e63389a225284a84b51e634ab02a43ad2070592e"
        );
    }

    /// Shared with packages/bid (TypeScript).
    #[test]
    fn price_commitment_vector() {
        assert_eq!(
            hex::encode(price_commitment(
                "7K3QX9MZ2P",
                "utest1refundexample",
                42_000_000,
                &[1; 32]
            )),
            PRICE_VECTOR
        );
        assert_ne!(
            price_commitment("7K3QX9MZ2P", "utest1refundexample", 42_000_001, &[1; 32]),
            price_commitment("7K3QX9MZ2P", "utest1refundexample", 42_000_000, &[1; 32]),
        );
    }

    const PRICE_VECTOR: &str = "3be46b6b494a7ca47c27b63d4e525861cca83f3c1d6cf25d17124ae1da5c542f";

    #[test]
    fn commitment_is_deterministic_and_salted() {
        let a = commitment("7K3QX9MZ2P", "utest1abc", &[1; 32]);
        assert_eq!(a, commitment("7K3QX9MZ2P", "utest1abc", &[1; 32]));
        assert_ne!(a, commitment("7K3QX9MZ2P", "utest1abc", &[2; 32]));
    }
}
