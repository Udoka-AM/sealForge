//! Escrow viewing keys and address.
//!
//! Orchard full viewing key = `ak ‖ nk ‖ rivk`. `ak` is the FROST group key
//! (even Y). `nk` and `rivk` grant viewing power, not spending power. They are
//! sampled once, with the OS CSPRNG, by the **coordinator** when the last
//! trustee completes the DKG (`sealforge-coordinator::routes::dkg::complete`
//! calls [`escrow_fvk`] with `OsRng`), and the resulting UFVK is stored
//! encrypted (`auctions.escrow_ufvk_enc`, `pgp_sym_encrypt` with `SF_DATA_KEY`).
//! ZIP 312 does not specify how `nk` and `rivk` are generated; who samples them
//! is a sealForge trust assumption, documented in docs/THREAT-MODEL.md.

use orchard::keys::{FullViewingKey, Scope};
use rand::{CryptoRng, RngCore};
use zcash_keys::{address::UnifiedAddress, keys::UnifiedFullViewingKey};
use zcash_protocol::consensus::{BlockHeight, NetworkType, NetworkUpgrade, Parameters};

/// Encoding parameters for a network. Address and key encodings depend only on
/// the network type, so activation heights are not needed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodingParams(pub NetworkType);

impl Parameters for EncodingParams {
    fn network_type(&self) -> NetworkType {
        self.0
    }

    fn activation_height(&self, _nu: NetworkUpgrade) -> Option<BlockHeight> {
        None
    }
}

use crate::EscrowError;

/// Build the escrow FVK from the group key and freshly sampled `nk` and `rivk`.
pub fn escrow_fvk<R: RngCore + CryptoRng>(
    ak: &[u8; 32],
    rng: &mut R,
) -> Result<FullViewingKey, EscrowError> {
    // Rejection-sample canonical field encodings. Clearing the top two bits
    // keeps both values below the Pallas moduli (~2^254), so this almost
    // always succeeds on the first try.
    for _ in 0..64 {
        let mut bytes = [0u8; 96];
        bytes[..32].copy_from_slice(ak);
        rng.fill_bytes(&mut bytes[32..]);
        bytes[63] &= 0x3f;
        bytes[95] &= 0x3f;
        if let Some(fvk) = FullViewingKey::from_bytes(&bytes) {
            return Ok(fvk);
        }
    }
    Err(EscrowError::KeyDerivation)
}

/// The `nk ‖ rivk` half of an escrow FVK (64 bytes). Sealed to each trustee
/// so it can rebuild the FVK from its own copy of the group key and check the
/// escrow address before reporting ready. Viewing power only, like the FVK.
pub fn view_secret(fvk: &FullViewingKey) -> [u8; 64] {
    let bytes = fvk.to_bytes();
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes[32..]);
    out
}

/// Rebuild an escrow FVK from the group key `ak` and a [`view_secret`].
pub fn escrow_fvk_from_parts(
    ak: &[u8; 32],
    view_secret: &[u8; 64],
) -> Result<FullViewingKey, EscrowError> {
    let mut bytes = [0u8; 96];
    bytes[..32].copy_from_slice(ak);
    bytes[32..].copy_from_slice(view_secret);
    FullViewingKey::from_bytes(&bytes).ok_or(EscrowError::KeyDerivation)
}

/// The escrow Unified Address (Orchard receiver, used by the Ironwood pool).
pub fn escrow_address<P: Parameters>(
    fvk: &FullViewingKey,
    params: &P,
) -> Result<String, EscrowError> {
    let receiver = fvk.address_at(0u32, Scope::External);
    let ua = UnifiedAddress::from_receivers(Some(receiver), None, None, None, None)
        .ok_or_else(|| EscrowError::Encoding("no shielded receiver".into()))?;
    Ok(ua.encode(params))
}

/// UFVK (kept private until settlement) and UIVK (published at close so anyone
/// can audit incoming bids without seeing outgoing refunds).
pub fn escrow_view_keys<P: Parameters>(
    fvk: &FullViewingKey,
    params: &P,
) -> Result<(String, String), EscrowError> {
    let ufvk = UnifiedFullViewingKey::from_orchard_fvk(fvk.clone())
        .map_err(|e| EscrowError::Encoding(format!("{e:?}")))?;
    let ufvk_str = ufvk
        .encode(params)
        .map_err(|e| EscrowError::Encoding(format!("{e:?}")))?;
    let uivk_str = ufvk
        .to_unified_incoming_viewing_key()
        .encode(params)
        .map_err(|e| EscrowError::Encoding(format!("{e:?}")))?;
    Ok((ufvk_str, uivk_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dkg, Threshold};
    use rand::rngs::OsRng;
    use zcash_protocol::consensus::NetworkType;

    #[test]
    fn the_escrow_address_rebuilds_from_the_group_key_and_view_secret() {
        let (_, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let ak = dkg::group_key_bytes(&pubkeys).unwrap();
        let params = EncodingParams(NetworkType::Test);
        let fvk = escrow_fvk(&ak, &mut OsRng).unwrap();
        let ua = escrow_address(&fvk, &params).unwrap();

        let rebuilt = escrow_fvk_from_parts(&ak, &view_secret(&fvk)).unwrap();
        assert_eq!(escrow_address(&rebuilt, &params).unwrap(), ua);

        // Another group key with the same view secret gives a different address,
        // so a matching address proves the escrow is bound to this group key.
        let (_, other) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let other_ak = dkg::group_key_bytes(&other).unwrap();
        if let Ok(fvk) = escrow_fvk_from_parts(&other_ak, &view_secret(&fvk)) {
            assert_ne!(escrow_address(&fvk, &params).unwrap(), ua);
        }
    }
}
