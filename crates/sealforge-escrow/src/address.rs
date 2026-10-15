//! Escrow viewing keys and address.
//!
//! Orchard full viewing key = `ak ‖ nk ‖ rivk`. `ak` is the FROST group key
//! (even Y). `nk` and `rivk` only grant viewing power and are generated once
//! by the organiser at DKG completion (ZIP 312), then stored encrypted by the
//! coordinator (`auctions.escrow_ufvk_enc`).

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
