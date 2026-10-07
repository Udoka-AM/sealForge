//! ZIP-321 payment URIs for bids.
//!
//! `zcash:<escrow_ua>?amount=<ZEC decimal>&memo=<base64url(memo)>`

use crate::{amount::COIN, Zatoshi};

/// Format zatoshi as a ZIP-321 decimal ZEC amount (no trailing zeros).
pub fn format_zec(amount: Zatoshi) -> String {
    let whole = amount.0 / COIN;
    let frac = amount.0 % COIN;
    if frac == 0 {
        return whole.to_string();
    }
    let frac = format!("{frac:08}");
    format!("{whole}.{}", frac.trim_end_matches('0'))
}

/// base64url without padding, as ZIP-321 requires for memos.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// Build a single-payment ZIP-321 URI.
pub fn payment_uri(address: &str, amount: Zatoshi, memo: &str) -> String {
    format!(
        "zcash:{address}?amount={}&memo={}",
        format_zec(amount),
        base64url(memo.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_amounts() {
        assert_eq!(format_zec(Zatoshi(100_000_000)), "1");
        assert_eq!(format_zec(Zatoshi(150_000_000)), "1.5");
        assert_eq!(format_zec(Zatoshi(1)), "0.00000001");
    }

    #[test]
    fn base64url_matches_rfc4648() {
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"\xfb\xff"), "-_8");
    }

    #[test]
    fn builds_uri() {
        let uri = payment_uri("utest1escrow", Zatoshi(42_000_000), "SF1");
        assert_eq!(uri, "zcash:utest1escrow?amount=0.42&memo=U0Yx");
    }
}
