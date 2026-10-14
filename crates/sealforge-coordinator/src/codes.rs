//! Random public codes (Crockford base32: no I, L, O, U).

use rand::{rngs::OsRng, Rng};
use sha2::{Digest, Sha256};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn random_code(len: usize) -> String {
    let mut rng = OsRng;
    (0..len)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Public auction code used in memos and URLs.
pub fn auction_code() -> String {
    random_code(sealforge_core::memo::AUCTION_CODE_LEN)
}

/// One-time trustee invite code (100 bits of entropy).
pub fn invite_code() -> String {
    random_code(20)
}

pub fn sha256(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auction_codes_are_valid_memo_codes() {
        for _ in 0..100 {
            assert!(sealforge_core::memo::is_valid_auction_code(&auction_code()));
        }
    }
}
