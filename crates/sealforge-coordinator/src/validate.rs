//! Input validation (docs/05-BACKEND-SCHEMA.md §5.7).

use std::collections::BTreeSet;

use sealforge_api::types::{CreateAuction, Network, TrusteeSlotInput, UpdateAuction};
use sealforge_core::amount::MAX_MONEY;
use zcash_keys::address::Address;
use zcash_protocol::consensus::NetworkType;

use crate::error::ApiError;

/// Minimum blocks between opening and closing (≈ 1 h at NU7's 25 s blocks).
pub const MIN_OPEN_BLOCKS: u32 = 144;

/// Address-encoding parameters for an auction's network.
pub type AddressParams = sealforge_escrow::address::EncodingParams;

pub fn params_for(network: Network) -> AddressParams {
    sealforge_escrow::address::EncodingParams(network_type(network))
}

fn network_type(network: Network) -> NetworkType {
    match network {
        Network::Mainnet => NetworkType::Main,
        Network::Testnet => NetworkType::Test,
        Network::Regtest => NetworkType::Regtest,
    }
}

/// A Unified Address for `network` with at least one shielded receiver.
pub fn shielded_ua(value: &str, network: Network) -> Result<(), ApiError> {
    match Address::decode(&params_for(network), value) {
        // Escrow funds live in the Ironwood pool, which uses Orchard receivers. Paying a
        // Sapling-only address would cross pools and reveal the amount on chain.
        Some(Address::Unified(ua)) if ua.has_orchard() => Ok(()),
        Some(Address::Unified(_)) => Err(ApiError::Validation(
            "address needs an Orchard receiver (Ironwood uses Orchard receivers)".into(),
        )),
        Some(_) => Err(ApiError::Validation(
            "a Unified Address is required (transparent and legacy addresses are not accepted)"
                .into(),
        )),
        None => Err(ApiError::Validation(format!(
            "not a valid {network:?} address"
        ))),
    }
}

fn title(value: &str) -> Result<(), ApiError> {
    let len = value.trim().chars().count();
    if (1..=120).contains(&len) {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "title must be 1–120 characters".into(),
        ))
    }
}

fn description(value: &str) -> Result<(), ApiError> {
    if value.chars().count() <= 4_000 {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "description must be at most 4,000 characters".into(),
        ))
    }
}

fn lot_image_url(value: &str) -> Result<(), ApiError> {
    if value.starts_with("https://") && value.len() <= 2_048 {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "lot image must be an https URL".into(),
        ))
    }
}

fn quantity(value: u32) -> Result<(), ApiError> {
    if (1..=100_000).contains(&value) {
        Ok(())
    } else {
        Err(ApiError::Validation("quantity must be 1–100,000".into()))
    }
}

fn reserve(value: u64) -> Result<(), ApiError> {
    if value <= MAX_MONEY {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "reserve exceeds the ZEC supply".into(),
        ))
    }
}

fn trustees(slots: &[TrusteeSlotInput]) -> Result<(), ApiError> {
    let ids: BTreeSet<u16> = slots.iter().map(|t| t.slot).collect();
    if slots.len() != 3 || ids != BTreeSet::from([1, 2, 3]) {
        return Err(ApiError::Validation(
            "exactly three trustees with slots 1, 2 and 3 are required".into(),
        ));
    }
    if slots
        .iter()
        .any(|t| !(1..=80).contains(&t.label.trim().chars().count()))
    {
        return Err(ApiError::Validation(
            "trustee labels must be 1–80 characters".into(),
        ));
    }
    Ok(())
}

pub fn create_auction(input: &CreateAuction) -> Result<(), ApiError> {
    title(&input.title)?;
    description(&input.description)?;
    if let Some(url) = &input.lot_image_url {
        lot_image_url(url)?;
    }
    quantity(input.quantity)?;
    reserve(input.reserve_zat)?;
    if input.close_height == 0 {
        return Err(ApiError::Validation("close height is required".into()));
    }
    shielded_ua(&input.payout_ua, input.network)?;
    trustees(&input.trustees)
}

pub fn update_auction(input: &UpdateAuction, network: Network) -> Result<(), ApiError> {
    if let Some(v) = &input.title {
        title(v)?;
    }
    if let Some(v) = &input.description {
        description(v)?;
    }
    if let Some(v) = &input.lot_image_url {
        lot_image_url(v)?;
    }
    if let Some(v) = input.quantity {
        quantity(v)?;
    }
    if let Some(v) = input.reserve_zat {
        reserve(v)?;
    }
    if input.close_height == Some(0) {
        return Err(ApiError::Validation("close height is required".into()));
    }
    if let Some(v) = &input.payout_ua {
        shielded_ua(v, network)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A testnet UA with an Orchard receiver (derived from a throwaway escrow key).
    pub const TESTNET_UA: &str = "utest10wa7f6qs6jevt8pl9vezqtrnm9qdkcccdyjtsam4sqgyq4mwvnvxaen2nnsd0yu47qh6a6s0payftzz4h86c6caev4qguam3huf3z7yu";

    #[test]
    fn unified_address_checks() {
        shielded_ua(TESTNET_UA, Network::Testnet).unwrap();
        assert!(
            shielded_ua(TESTNET_UA, Network::Mainnet).is_err(),
            "wrong network"
        );
        assert!(
            shielded_ua("tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd", Network::Testnet).is_err(),
            "transparent"
        );
        assert!(shielded_ua("not-an-address", Network::Testnet).is_err());
    }

    #[test]
    fn trustee_slots() {
        let slot = |slot, label: &str| TrusteeSlotInput {
            slot,
            label: label.into(),
        };
        trustees(&[
            slot(1, "Organiser"),
            slot(2, "Auditor"),
            slot(3, "Community"),
        ])
        .unwrap();
        assert!(trustees(&[slot(1, "a"), slot(2, "b")]).is_err());
        assert!(trustees(&[slot(1, "a"), slot(1, "b"), slot(3, "c")]).is_err());
        assert!(trustees(&[slot(1, "a"), slot(2, " "), slot(3, "c")]).is_err());
    }
}
