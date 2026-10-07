//! Canonical refund denominations (ZIP 318 style): {1, 2, 5} × 10^k zatoshi.
//!
//! Refunds are split into canonical notes so their amounts don't fingerprint
//! the bid. Anything below [`MIN_DENOMINATION`] becomes one remainder note
//! (PRD open question Q7).

use crate::Zatoshi;

/// Smallest canonical denomination: 0.0001 ZEC.
pub const MIN_DENOMINATION: u64 = 10_000;

/// Largest canonical denomination: 5,000 ZEC.
pub const MAX_DENOMINATION: u64 = 5_000 * crate::amount::COIN;

/// All canonical denominations, largest first.
pub fn canonical_denominations() -> Vec<u64> {
    let mut out = Vec::new();
    let mut base = MIN_DENOMINATION;
    while base <= MAX_DENOMINATION {
        for m in [1, 2, 5] {
            let d = base * m;
            if d <= MAX_DENOMINATION {
                out.push(d);
            }
        }
        base *= 10;
    }
    out.reverse();
    out
}

/// Returns true if `amount` is a canonical denomination.
pub fn is_canonical(amount: Zatoshi) -> bool {
    canonical_denominations().contains(&amount.0)
}

/// Greedy split of `amount` into canonical notes (largest first), plus an
/// optional remainder below [`MIN_DENOMINATION`]. The notes always sum to `amount`.
pub fn split(amount: Zatoshi) -> (Vec<Zatoshi>, Option<Zatoshi>) {
    let mut left = amount.0;
    let mut notes = Vec::new();
    for d in canonical_denominations() {
        while left >= d {
            notes.push(Zatoshi(d));
            left -= d;
        }
    }
    let remainder = (left > 0).then_some(Zatoshi(left));
    (notes, remainder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sums_to_amount() {
        for amount in [
            0,
            1,
            9_999,
            10_000,
            123_456_789,
            4_200_000_000,
            987_654_321_012,
        ] {
            let (notes, rem) = split(Zatoshi(amount));
            let sum: u64 = notes.iter().map(|z| z.0).sum::<u64>() + rem.map_or(0, |r| r.0);
            assert_eq!(sum, amount);
            assert!(notes.iter().all(|n| is_canonical(*n)));
            assert!(rem.is_none_or(|r| r.0 < MIN_DENOMINATION));
        }
    }

    #[test]
    fn example_split() {
        // 0.37 ZEC = 0.2 + 0.1 + 0.05 + 0.02
        let (notes, rem) = split(Zatoshi(37_000_000));
        assert_eq!(
            notes,
            vec![
                Zatoshi(20_000_000),
                Zatoshi(10_000_000),
                Zatoshi(5_000_000),
                Zatoshi(2_000_000)
            ]
        );
        assert_eq!(rem, None);
    }
}
