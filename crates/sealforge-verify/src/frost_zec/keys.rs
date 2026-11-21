// Vendored from frost-zec (https://github.com/OpenPGP-Wallet/frost-zec) at commit
// c417ced, MIT License, Copyright (c) 2026 Khadim. See ../../LICENSE-frost-zec.
// Kept as close to upstream as possible so the shared signer checks (frost-zec
// spec § 6) and tamper cases (§ 8.2, frost-zec#7) stay comparable. Local
// changes are marked "sealForge:".

//! The group's Orchard-protocol keys, derived as ZIP 2005 specifies for FROST.
//!
//! The spend validating key `ak` is the FROST group key. Everything else comes
//! from a value `sk` that the participants agree on (`use_qsk = true` in
//! ZIP 2005 § 4.2.3 'Orchard Key Components'):
//!
//! ```text
//! nk   = ToBase(PRF^expand_sk([0x07]))
//! qsk  = truncate_32(PRF^expand_sk([0x0C]))
//! qk   = BLAKE3.derive_key("Zcash ZIP 2005 qk-derivation v1", qsk)
//! rivk = ToScalar(PRF^expand_qk([0x0D] || ak || nk))
//! ```
//!
//! Notes received by these keys keep the quantum recovery path of ZIP 2005, as
//! long as qsk is kept. zcash-sign instead builds the full viewing key with a
//! constructor that gives that path up.

use std::error::Error;

use orchard::keys::FullViewingKey;
use zcash_address::unified::{self, Container, Encoding};
use zcash_protocol::consensus::NetworkType;

// sealForge: the ZIP 2005 key derivation (`derive`, `contributory_sk`, ...)
// is removed. sealForge's escrow keys come from its own DKG; only parsing a
// viewing key and encoding an address are needed here.

/// The unified full viewing key, with its Orchard component only (Ironwood
/// uses the same Orchard-protocol keys).
pub fn ufvk(fvk: &FullViewingKey, network: NetworkType) -> Result<String, Box<dyn Error>> {
    let ufvk = unified::Ufvk::try_from_items(
        unified::Revision::R0,
        vec![unified::Uitem::Data(unified::Fvk::Orchard(fvk.to_bytes()))],
    )?;
    Ok(ufvk.encode(&network))
}

/// The network and the Orchard full viewing key of a unified full viewing key.
pub fn parse_ufvk(text: &str) -> Result<(NetworkType, FullViewingKey), Box<dyn Error>> {
    let (network, _, ufvk) = unified::Ufvk::decode(text.trim())?;
    for item in ufvk.items() {
        if let unified::Fvk::Orchard(bytes) = item {
            let fvk =
                FullViewingKey::from_bytes(&bytes).ok_or("invalid Orchard full viewing key")?;
            return Ok((network, fvk));
        }
    }
    Err("the viewing key has no Orchard component".into())
}

/// An Ironwood address as a unified address on `network`.
pub fn encode_address(
    address: &orchard::Address,
    network: NetworkType,
) -> Result<String, Box<dyn Error>> {
    let ua = unified::Address::try_from_items(
        unified::Revision::R0,
        vec![unified::Uitem::Data(unified::Receiver::Orchard(
            address.to_raw_address_bytes(),
        ))],
    )?;
    Ok(ua.encode(&network))
}
