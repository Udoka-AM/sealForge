//! The browser signer rebuilds the escrow address with the vendored
//! `keys::encode_address`; it must be byte for byte the address
//! `sealforge_escrow::address::escrow_address` gives trustees and bidders.

use rand::rngs::OsRng;
use sealforge_escrow::{
    address::{escrow_address, escrow_fvk, EncodingParams},
    dkg, Threshold,
};
use sealforge_verify::frost_zec::keys;
use zcash_protocol::consensus::NetworkType;

#[test]
fn the_vendored_encoding_gives_the_escrow_address() {
    let (_, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
    let fvk = escrow_fvk(&dkg::group_key_bytes(&pubkeys).unwrap(), &mut OsRng).unwrap();
    for network in [NetworkType::Main, NetworkType::Test, NetworkType::Regtest] {
        assert_eq!(
            keys::encode_address(
                &fvk.address_at(0u32, orchard::keys::Scope::External),
                network
            )
            .unwrap(),
            escrow_address(&fvk, &EncodingParams(network)).unwrap(),
            "{network:?}"
        );
    }
}
