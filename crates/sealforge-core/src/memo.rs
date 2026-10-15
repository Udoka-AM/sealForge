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
}
