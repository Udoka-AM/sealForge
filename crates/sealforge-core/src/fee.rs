//! sealForge's protocol fee: a share of an auction's proceeds, paid as a
//! second output of the payout transaction. The rate is fixed per auction when
//! it is created and published with it; anyone recomputes the fee from the
//! published result. Pure: the coordinator, trustees and auditors all call it.

/// Basis points in one whole (100%).
pub const BPS_DENOMINATOR: u64 = 10_000;

/// The highest rate an auction may carry: 10%.
pub const MAX_FEE_BPS: u16 = 1_000;

/// The fee owed on `proceeds_zat` at `fee_bps`, rounded down to the zatoshi.
/// Zero when the rate is zero. Rates above [`MAX_FEE_BPS`] are clamped to it.
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
        // Clamped to 10%.
        assert_eq!(protocol_fee_zat(1_000_000, 9_999), 100_000);
    }
}
