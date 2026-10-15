//! Bid memo codec, v1.
//!
//! Grammar (ZIP-302 text memo, ≤ 512 bytes):
//! `SF1:<auction_code>:<commitment-hex64>:<refund_ua>`
//!
//! Address validation (network, shielded receiver) happens in the
//! coordinator, where the Zcash address crates are available.

use thiserror::Error;

/// Memo prefix for version 1.
pub const PREFIX_V1: &str = "SF1";

/// Maximum memo length in bytes (ZIP-302).
pub const MAX_MEMO_BYTES: usize = 512;

/// Length of a public auction code (Crockford base32).
pub const AUCTION_CODE_LEN: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidMemo {
    pub auction_code: String,
    pub commitment: [u8; 32],
    pub refund_ua: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoError {
    #[error("memo exceeds {MAX_MEMO_BYTES} bytes")]
    TooLong,
    #[error("unknown memo version or prefix")]
    BadPrefix,
    #[error("memo does not have 4 fields")]
    BadShape,
    #[error("invalid auction code")]
    BadAuctionCode,
    #[error("invalid commitment")]
    BadCommitment,
    #[error("missing refund address")]
    MissingRefundAddress,
}

/// Prefix of the organiser's fee-buffer deposit memo: `SF1FEE:<auction_code>`.
/// The deposit pays ZIP-317 fees for refunds and the payout (PRD Q8); what
/// isn't spent on fees returns to the organiser with the payout.
pub const FEE_BUFFER_PREFIX: &str = "SF1FEE";

/// The fee-buffer memo for an auction.
pub fn fee_buffer_memo(auction_code: &str) -> Result<String, MemoError> {
    if !is_valid_auction_code(auction_code) {
        return Err(MemoError::BadAuctionCode);
    }
    Ok(format!("{FEE_BUFFER_PREFIX}:{auction_code}"))
}

/// The auction code of a fee-buffer deposit memo, if `memo` is one.
pub fn parse_fee_buffer_memo(memo: &str) -> Option<&str> {
    let code = memo
        .trim_end_matches('\0')
        .strip_prefix(FEE_BUFFER_PREFIX)?
        .strip_prefix(':')?;
    is_valid_auction_code(code).then_some(code)
}

/// Prefix of a commit-reveal reveal memo (PRD F13):
/// `SF1R:<auction_code>:<commitment-hex64>:<price_zat>:<salt-hex64>`.
pub const REVEAL_PREFIX: &str = "SF1R";

/// A bidder's reveal, sent to the escrow after the bidding close.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealMemo {
    pub auction_code: String,
    /// The commitment in the matching deposit's memo.
    pub commitment: [u8; 32],
    pub price_zat: u64,
    pub salt: [u8; 32],
}

impl RevealMemo {
    pub fn encode(&self) -> Result<String, MemoError> {
        if !is_valid_auction_code(&self.auction_code) {
            return Err(MemoError::BadAuctionCode);
        }
        Ok(format!(
            "{REVEAL_PREFIX}:{}:{}:{}:{}",
            self.auction_code,
            hex::encode(self.commitment),
            self.price_zat,
            hex::encode(self.salt)
        ))
    }

    pub fn decode(memo: &str) -> Result<Self, MemoError> {
        let memo = memo.trim_end_matches('\0');
        let parts: Vec<&str> = memo.split(':').collect();
        let [prefix, code, commitment, price, salt] = parts[..] else {
            return Err(MemoError::BadShape);
        };
        if prefix != REVEAL_PREFIX {
            return Err(MemoError::BadPrefix);
        }
        if !is_valid_auction_code(code) {
            return Err(MemoError::BadAuctionCode);
        }
        let hex32 = |v: &str| -> Option<[u8; 32]> { hex::decode(v).ok()?.try_into().ok() };
        let commitment = hex32(commitment).ok_or(MemoError::BadCommitment)?;
        let salt = hex32(salt).ok_or(MemoError::BadCommitment)?;
        // Plain decimal zatoshi, no sign, no leading zeros.
        if price.is_empty() || (price.len() > 1 && price.starts_with('0')) {
            return Err(MemoError::BadShape);
        }
        let price_zat: u64 = price.parse().map_err(|_| MemoError::BadShape)?;
        Ok(Self {
            auction_code: code.to_owned(),
            commitment,
            price_zat,
            salt,
        })
    }
}

/// Returns true if `code` is a valid 10-char Crockford base32 auction code.
pub fn is_valid_auction_code(code: &str) -> bool {
    code.len() == AUCTION_CODE_LEN
        && code
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'H' | b'J' | b'K' | b'M' | b'N' | b'P'..=b'T' | b'V'..=b'Z'))
}

impl BidMemo {
    /// Encode to the v1 text memo.
    pub fn encode(&self) -> Result<String, MemoError> {
        if !is_valid_auction_code(&self.auction_code) {
            return Err(MemoError::BadAuctionCode);
        }
        if self.refund_ua.is_empty() {
            return Err(MemoError::MissingRefundAddress);
        }
        let memo = format!(
            "{PREFIX_V1}:{}:{}:{}",
            self.auction_code,
            hex::encode(self.commitment),
            self.refund_ua
        );
        if memo.len() > MAX_MEMO_BYTES {
            return Err(MemoError::TooLong);
        }
        Ok(memo)
    }

    /// Parse a v1 text memo. Trailing NUL padding (as returned by wallets) is ignored.
    pub fn decode(memo: &str) -> Result<Self, MemoError> {
        let memo = memo.trim_end_matches('\0');
        if memo.len() > MAX_MEMO_BYTES {
            return Err(MemoError::TooLong);
        }
        let mut parts = memo.splitn(4, ':');
        let (Some(prefix), Some(code), Some(commitment), Some(refund_ua)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(MemoError::BadShape);
        };
        if prefix != PREFIX_V1 {
            return Err(MemoError::BadPrefix);
        }
        if !is_valid_auction_code(code) {
            return Err(MemoError::BadAuctionCode);
        }
        let commitment: [u8; 32] = hex::decode(commitment)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or(MemoError::BadCommitment)?;
        if refund_ua.is_empty() || refund_ua.contains(':') {
            return Err(MemoError::MissingRefundAddress);
        }
        Ok(Self {
            auction_code: code.to_owned(),
            commitment,
            refund_ua: refund_ua.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BidMemo {
        BidMemo {
            auction_code: "7K3QX9MZ2P".into(),
            commitment: [0xab; 32],
            refund_ua: "utest1exampleaddress".into(),
        }
    }

    #[test]
    fn round_trip() {
        let m = sample();
        let encoded = m.encode().unwrap();
        assert!(encoded.starts_with("SF1:7K3QX9MZ2P:abab"));
        assert_eq!(BidMemo::decode(&encoded).unwrap(), m);
    }

    #[test]
    fn ignores_nul_padding() {
        let encoded = format!("{}\0\0\0", sample().encode().unwrap());
        assert_eq!(BidMemo::decode(&encoded).unwrap(), sample());
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            BidMemo::decode("SF2:7K3QX9MZ2P:00:u1"),
            Err(MemoError::BadPrefix)
        );
        assert_eq!(BidMemo::decode("SF1:7K3QX9MZ2P"), Err(MemoError::BadShape));
        assert_eq!(
            BidMemo::decode(&format!("SF1:7K3QX9MZ2I:{}:u1", "00".repeat(32))),
            Err(MemoError::BadAuctionCode)
        );
        assert_eq!(
            BidMemo::decode("SF1:7K3QX9MZ2P:zz:u1"),
            Err(MemoError::BadCommitment)
        );
        assert_eq!(BidMemo::decode(&"x".repeat(600)), Err(MemoError::TooLong));
    }

    #[test]
    fn fee_buffer_memo_round_trips_and_is_not_a_bid() {
        let memo = fee_buffer_memo("JTY3AX0SMC").unwrap();
        assert_eq!(memo, "SF1FEE:JTY3AX0SMC");
        assert_eq!(parse_fee_buffer_memo(&memo), Some("JTY3AX0SMC"));
        assert!(BidMemo::decode(&memo).is_err());
        assert_eq!(parse_fee_buffer_memo("SF1FEE:bad"), None);
        assert_eq!(parse_fee_buffer_memo("SF1:JTY3AX0SMC"), None);
        assert!(fee_buffer_memo("nope").is_err());
    }

    #[test]
    fn reveal_memo_round_trips() {
        let r = RevealMemo {
            auction_code: "7K3QX9MZ2P".into(),
            commitment: [0xab; 32],
            price_zat: 42_000_000,
            salt: [1; 32],
        };
        let memo = r.encode().unwrap();
        assert!(memo.starts_with("SF1R:7K3QX9MZ2P:abab"));
        assert_eq!(RevealMemo::decode(&memo).unwrap(), r);
        assert_eq!(RevealMemo::decode(&format!("{memo}\0\0")).unwrap(), r);
        assert!(BidMemo::decode(&memo).is_err(), "a reveal is never a bid");
        assert!(RevealMemo::decode(&memo.replace(":42000000:", ":042000000:")).is_err());
        assert!(RevealMemo::decode(&memo.replace(":42000000:", ":-1:")).is_err());
        assert!(RevealMemo::decode("SF1:7K3QX9MZ2P:00:00").is_err());
    }
}
