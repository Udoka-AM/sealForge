//! sealForge's protocol fee: a share of an auction's proceeds, paid as a
//! second output of the payout transaction. The rate is fixed per auction
//! before it opens and published with it; anyone recomputes the fee from the
//! published result. Pure: the coordinator, trustees and auditors all call it.

/// Basis points in one whole (100%).
pub const BPS_DENOMINATOR: u64 = 10_000;

/// The lowest non-zero rate an auction may carry: 5%.
pub const MIN_FEE_BPS: u16 = 500;

/// The highest rate an auction may carry: 15%.
pub const MAX_FEE_BPS: u16 = 1_500;

/// Whether an auction may carry `fee_bps`: no fee at all, or a rate from
/// [`MIN_FEE_BPS`] to [`MAX_FEE_BPS`].
pub fn fee_rate_allowed(fee_bps: u16) -> bool {
    fee_bps == 0 || (MIN_FEE_BPS..=MAX_FEE_BPS).contains(&fee_bps)
}

/// The fee owed on `proceeds_zat` at `fee_bps`, rounded down to the zatoshi.
/// Zero when the rate is zero. Rates above [`MAX_FEE_BPS`] are clamped to it,
/// so a published rate above the cap never yields a larger fee.
pub fn protocol_fee_zat(proceeds_zat: u64, fee_bps: u16) -> u64 {
    let bps = u64::from(fee_bps.min(MAX_FEE_BPS));
    ((u128::from(proceeds_zat) * u128::from(bps)) / u128::from(BPS_DENOMINATOR)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_is_a_rounded_down_share_of_proceeds() {
        assert_eq!(protocol_fee_zat(20_000_000, 0), 0);
        assert_eq!(protocol_fee_zat(20_000_000, 250), 500_000); // 2.5%
        assert_eq!(protocol_fee_zat(1_999, 500), 99); // 99.95 rounds down
        assert_eq!(protocol_fee_zat(u64::MAX, 1_000), u64::MAX / 10);
        assert_eq!(protocol_fee_zat(20_000_000, 1_500), 3_000_000); // 15%
        assert_eq!(protocol_fee_zat(1_000_000, 9_999), 150_000); // clamped to 15%
    }

    #[test]
    fn allowed_rates_are_none_or_five_to_fifteen_percent() {
        assert!(fee_rate_allowed(0));
        assert!(!fee_rate_allowed(1));
        assert!(!fee_rate_allowed(499));
        assert!(fee_rate_allowed(500));
        assert!(fee_rate_allowed(1_000));
        assert!(fee_rate_allowed(1_500));
        assert!(!fee_rate_allowed(1_501));
    }
}
