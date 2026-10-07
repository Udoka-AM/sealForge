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

    #[test]
    fn commitment_is_deterministic_and_salted() {
        let a = commitment("7K3QX9MZ2P", "utest1abc", &[1; 32]);
        assert_eq!(a, commitment("7K3QX9MZ2P", "utest1abc", &[1; 32]));
        assert_ne!(a, commitment("7K3QX9MZ2P", "utest1abc", &[2; 32]));
    }
}
