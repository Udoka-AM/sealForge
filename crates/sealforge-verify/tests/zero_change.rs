//! A real payout from the regtest drill for the protocol fee (issue #74): one
//! escrow note swept into the organiser's payout plus the fee, with change of
//! exactly zero. The wallet still adds a zero-value change note (encrypted with
//! no outgoing key), so the bundle has three actions for two payments. The
//! check must read that note with the group's internal incoming key and allow
//! it, and only it.

use std::{fs, path::Path};

use sealforge_verify::frost_zec::{check, keys, pczt_io};
use zcash_protocol::consensus::NetworkType;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/zero-change");

#[test]
fn a_payout_with_zero_change_and_a_fee_output_passes() {
    let ufvk = fs::read_to_string(Path::new(DIR).join("escrow.ufvk")).unwrap();
    let (network, fvk) = keys::parse_ufvk(&ufvk).unwrap();
    assert_eq!(network, NetworkType::Regtest);
    let pczt = pczt_io::parse(&fs::read(Path::new(DIR).join("payout.pczt")).unwrap()).unwrap();
    let report = check::analyze_at(&pczt, &fvk, network, 0)
        .unwrap_or_else(|e| panic!("the payout is refused: {e}"));
    let text = report.lines.join("\n");
    assert!(text.contains("change 0 "), "{text}");
    assert_eq!(text.matches("PAY ").count(), 2, "{text}");
    assert!(text.contains("PAY 0.00500000"), "{text}");
}
