//! The address recovered from a published UIVK is the one the coordinator
//! shows bidders, so the auditor can tell when the two don't belong together.

use rand::rngs::OsRng;
use sealforge_escrow::{
    address::{escrow_address, escrow_fvk, escrow_view_keys, EncodingParams},
    dkg, Threshold,
};
use sealforge_scan::ChainParams;
use zcash_protocol::consensus::NetworkType;

fn escrow() -> (String, String) {
    let (_, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
    let ak = dkg::group_key_bytes(&pubkeys).unwrap();
    let fvk = escrow_fvk(&ak, &mut OsRng).unwrap();
    let params = EncodingParams(NetworkType::Test);
    let ua = escrow_address(&fvk, &params).unwrap();
    let (_, uivk) = escrow_view_keys(&fvk, &params).unwrap();
    (ua, uivk)
}

#[test]
fn the_published_viewing_key_receives_at_the_escrow_address() {
    let (ua, uivk) = escrow();
    assert_eq!(
        sealforge_scan::escrow_address(&ChainParams::Testnet, &uivk).unwrap(),
        ua
    );
}

#[test]
fn another_escrow_s_viewing_key_does_not() {
    let (ua, _) = escrow();
    let (_, other_uivk) = escrow();
    assert_ne!(
        sealforge_scan::escrow_address(&ChainParams::Testnet, &other_uivk).unwrap(),
        ua
    );
}
