//! The shared tamper cases of frost-zec (spec § 8.2, frost-zec#7), run against
//! sealForge's trustee checks. Ported from frost-zec's `tests/tamper_vectors.rs`
//! at c417ced (MIT, Copyright (c) 2026 Khadim): the accepted PCZT must be
//! accepted, show exactly the approved payments and have the given sighash; the
//! refused one must be refused with one of the case's classes.

use std::fs;
use std::path::Path;

use sealforge_verify::frost_zec::{check, keys, pczt_io};
use sealforge_verify::refusal_class;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zcash_protocol::consensus::NetworkType;
use zcash_protocol::memo::MemoBytes;

const DIR: &str = "tests/vectors/tamper";

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The payments shown, as (amount line, memo line) pairs.
fn shown_payments(report: &check::Report) -> Vec<(String, String)> {
    let lines = &report.lines;
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("PAY "))
        .map(|(i, line)| {
            let memo = lines.get(i + 1).map(|m| m.trim().to_string());
            (line.clone(), memo.unwrap_or_default())
        })
        .collect()
}

/// The approved payments as this co-signer would show them.
fn expected_payments(case: &Value, network: NetworkType) -> Vec<(String, String)> {
    case["approved"]["payments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let amount = check::amount(p["amount"].as_u64().unwrap(), network);
            let address = p["address"].as_str().unwrap();
            let memo = match p.get("memo").and_then(Value::as_str) {
                Some(text) => MemoBytes::from_bytes(text.as_bytes()).unwrap(),
                None => MemoBytes::empty(),
            };
            (
                format!("PAY {amount} to {address}"),
                check::describe_memo(memo.as_array()),
            )
        })
        .collect()
}

#[test]
fn every_shared_tamper_case_passes() {
    let mut ids: Vec<_> = fs::read_dir(Path::new(DIR))
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            path.is_dir().then_some(path)
        })
        .collect();
    ids.sort();
    assert!(ids.len() >= 17, "{} cases", ids.len());
    for dir in ids {
        let case: Value =
            serde_json::from_slice(&fs::read(dir.join("case.json")).unwrap()).unwrap();
        let id = case["id"].as_str().unwrap();
        assert_eq!(case["format"], 1, "{id}");
        let network = match case["network"].as_str().unwrap() {
            "main" => NetworkType::Main,
            "test" => NetworkType::Test,
            other => panic!("{id}: network {other}"),
        };
        let (ufvk_network, fvk) =
            keys::parse_ufvk(case["group"]["ufvk"].as_str().unwrap()).unwrap();
        assert_eq!(ufvk_network, network, "{id}");
        let clock = case["clock"].as_u64().unwrap();
        let classes: Vec<&str> = case["class"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();

        let accepted = fs::read(dir.join("accepted.pczt")).unwrap();
        assert_eq!(sha256(&accepted), case["accepted"]["sha256"], "{id}");
        let pczt = pczt_io::parse(&accepted).unwrap();
        let report = check::analyze_at(&pczt, &fvk, network, clock)
            .unwrap_or_else(|e| panic!("{id}: the accepted PCZT is refused: {e}"));
        assert_eq!(
            shown_payments(&report),
            expected_payments(&case, network),
            "{id}: the accepted PCZT does not show the approved payments"
        );
        assert_eq!(
            hex::encode(pczt_io::sighash(&pczt).unwrap()),
            case["accepted"]["sighash"],
            "{id}: sighash"
        );

        let Some(refused_sha) = case.get("refused").map(|r| &r["sha256"]) else {
            assert!(!dir.join("refused.pczt").exists(), "{id}");
            continue;
        };
        let refused = fs::read(dir.join("refused.pczt")).unwrap();
        assert_eq!(&Value::from(sha256(&refused)), refused_sha, "{id}");
        let class = match pczt_io::parse(&refused)
            .map_err(|e| e.to_string())
            .and_then(|pczt| {
                check::analyze_at(&pczt, &fvk, network, clock).map_err(|e| e.to_string())
            }) {
            Err(error) => {
                let class = refusal_class(&error);
                assert_ne!(class, "unclassified", "{id}: no class for \"{error}\"");
                class
            }
            // Accepted, but showing another payment than the approved one:
            // the user, comparing, refuses it.
            Ok(report) if shown_payments(&report) != expected_payments(&case, network) => "payment",
            Ok(report) => panic!("{id}: the refused PCZT is accepted: {:?}", report.lines),
        };
        assert!(
            classes.contains(&class),
            "{id}: refused as {class}, expected one of {classes:?}"
        );
    }
}
