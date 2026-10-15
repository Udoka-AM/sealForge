//! Trustee signing in the browser (`/trustee/{code}`, task 3.10, S16).
//!
//! The same batch helpers the CLI uses (`sealforge_escrow::signing`), exposed
//! with hex/JSON strings. The page keeps the key package and nonces in memory
//! only: nothing here touches storage or the network.

use std::collections::BTreeMap;

use rand::rngs::OsRng;
use sealforge_escrow::{frost::keys::KeyPackage, signing};
use serde::Serialize;
use wasm_bindgen::prelude::*;

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn js<T>(r: Res<T>) -> Result<T, JsError> {
    r.map_err(|e| JsError::new(&e))
}

fn hex32(value: &str) -> Res<[u8; 32]> {
    hex::decode(value)
        .map_err(err)?
        .try_into()
        .map_err(|_| "expected 32 bytes".to_string())
}

fn key_package(hex_value: &str) -> Res<KeyPackage> {
    hex::decode(hex_value.trim())
        .ok()
        .and_then(|b| KeyPackage::deserialize(&b).ok())
        .ok_or_else(|| "this isn't a sealForge key share (key-package.hex)".to_string())
}

#[derive(Serialize)]
struct KeyInfo {
    /// Trustee slot (FROST identifier).
    slot: u16,
    /// Escrow group key (ak), hex.
    group_key: String,
}

/// Slot and group key of a key share, to check it against the identity file.
#[wasm_bindgen]
pub fn key_info(key_package_hex: &str) -> Result<String, JsError> {
    js(key_info_inner(key_package_hex))
}

fn key_info_inner(key_package_hex: &str) -> Res<String> {
    let kp = key_package(key_package_hex)?;
    let id = kp.identifier().serialize();
    // Identifiers are small integers, little-endian in a 32-byte scalar.
    if id[2..].iter().any(|b| *b != 0) {
        return Err("unexpected trustee identifier".into());
    }
    let info = KeyInfo {
        slot: u16::from_le_bytes([id[0], id[1]]),
        group_key: hex::encode(kp.verifying_key().serialize().map_err(err)?),
    };
    serde_json::to_string(&info).map_err(err)
}

#[derive(Serialize)]
struct Round1 {
    /// Secret: keep in memory until round 2, then drop.
    nonces: Vec<String>,
    commitments: Vec<String>,
}

/// Round 1: nonces and commitments for `spends` escrow spends.
#[wasm_bindgen]
pub fn commit_batch(key_package_hex: &str, spends: u32) -> Result<String, JsError> {
    js(commit_batch_inner(key_package_hex, spends))
}

fn commit_batch_inner(key_package_hex: &str, spends: u32) -> Res<String> {
    let kp = key_package(key_package_hex)?;
    let round1 = signing::commit_batch(&kp, spends as usize, &mut OsRng).map_err(err)?;
    serde_json::to_string(&Round1 {
        nonces: round1.nonces.iter().map(hex::encode).collect(),
        commitments: round1.commitments.iter().map(hex::encode).collect(),
    })
    .map_err(err)
}

/// Round 2: one signature share per spend.
///
/// - `alphas_json`: `["<hex α>", …]` in spend order
/// - `signer_commitments_json`: `{"<slot>": ["<hex>", …], …}` for both signers
/// - `nonces_json`: this trustee's round 1 nonces
#[wasm_bindgen]
pub fn sign_batch(
    key_package_hex: &str,
    sighash_hex: &str,
    alphas_json: &str,
    signer_commitments_json: &str,
    nonces_json: &str,
) -> Result<String, JsError> {
    js(sign_batch_inner(
        key_package_hex,
        sighash_hex,
        alphas_json,
        signer_commitments_json,
        nonces_json,
    ))
}

fn sign_batch_inner(
    key_package_hex: &str,
    sighash_hex: &str,
    alphas_json: &str,
    signer_commitments_json: &str,
    nonces_json: &str,
) -> Res<String> {
    let kp = key_package(key_package_hex)?;
    let alphas = serde_json::from_str::<Vec<String>>(alphas_json)
        .map_err(err)?
        .iter()
        .map(|a| hex32(a))
        .collect::<Result<Vec<_>, _>>()?;
    let commitments = serde_json::from_str::<BTreeMap<u16, Vec<String>>>(signer_commitments_json)
        .map_err(err)?
        .into_iter()
        .map(|(slot, values)| {
            Ok((
                slot,
                values
                    .iter()
                    .map(hex::decode)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(err)?,
            ))
        })
        .collect::<Res<BTreeMap<_, _>>>()?;
    let nonces = serde_json::from_str::<Vec<String>>(nonces_json)
        .map_err(err)?
        .iter()
        .map(hex::decode)
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let shares = signing::sign_batch(&kp, hex32(sighash_hex)?, &alphas, &commitments, &nonces)
        .map_err(err)?;
    serde_json::to_string(&shares.iter().map(hex::encode).collect::<Vec<_>>()).map_err(err)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rand::{rngs::OsRng, RngCore};
    use sealforge_escrow::{
        dkg,
        frost::{round1::SigningCommitments, round2::SignatureShare},
        signing::{self, SpendAuthRequest},
        Threshold,
    };

    use super::*;

    fn alpha() -> [u8; 32] {
        use pasta_curves_ff::{FromUniformBytes, PrimeField};
        let mut wide = [0u8; 64];
        OsRng.fill_bytes(&mut wide);
        pasta_curves::Fq::from_uniform_bytes(&wide).to_repr()
    }

    mod pasta_curves_ff {
        pub use pasta_curves::group::ff::{FromUniformBytes, PrimeField};
    }

    /// Two trustees sign a two-spend batch through the JSON API; the
    /// aggregated signatures verify as Orchard spend authorizations.
    #[test]
    fn browser_api_produces_valid_spend_signatures() {
        let threshold = Threshold {
            min_signers: 2,
            max_signers: 3,
        };
        let (keys, pubkeys) = dkg::run_local(threshold, &mut OsRng).unwrap();
        let ak = dkg::group_key_bytes(&pubkeys).unwrap();
        let hex_key =
            |slot: u16| hex::encode(keys[&dkg::identifier(slot).unwrap()].serialize().unwrap());

        let info: serde_json::Value =
            serde_json::from_str(&key_info_inner(&hex_key(3)).unwrap()).unwrap();
        assert_eq!(info["slot"], 3);
        assert_eq!(info["group_key"], hex::encode(ak));

        let sighash = [9u8; 32];
        let alphas = [alpha(), alpha()];
        let alphas_json =
            serde_json::to_string(&alphas.iter().map(hex::encode).collect::<Vec<_>>()).unwrap();

        let round1: BTreeMap<u16, serde_json::Value> = [1u16, 3]
            .into_iter()
            .map(|slot| {
                let r = commit_batch_inner(&hex_key(slot), 2).unwrap();
                (slot, serde_json::from_str(&r).unwrap())
            })
            .collect();
        let commitments: BTreeMap<u16, serde_json::Value> = round1
            .iter()
            .map(|(slot, r)| (*slot, r["commitments"].clone()))
            .collect();
        let commitments_json = serde_json::to_string(&commitments).unwrap();

        let shares: BTreeMap<u16, Vec<String>> = round1
            .iter()
            .map(|(slot, r)| {
                let out = sign_batch_inner(
                    &hex_key(*slot),
                    &hex::encode(sighash),
                    &alphas_json,
                    &commitments_json,
                    &r["nonces"].to_string(),
                )
                .unwrap();
                (*slot, serde_json::from_str(&out).unwrap())
            })
            .collect();

        for (i, a) in alphas.iter().enumerate() {
            let request = SpendAuthRequest { sighash, alpha: *a };
            let mut c = BTreeMap::new();
            let mut s = BTreeMap::new();
            for slot in [1u16, 3] {
                let id = dkg::identifier(slot).unwrap();
                let ci = commitments[&slot][i].as_str().unwrap();
                c.insert(
                    id,
                    SigningCommitments::deserialize(&hex::decode(ci).unwrap()).unwrap(),
                );
                s.insert(
                    id,
                    SignatureShare::deserialize(&hex::decode(&shares[&slot][i]).unwrap()).unwrap(),
                );
            }
            let package = signing::signing_package(&request, c);
            let sig = signing::aggregate(&request, &package, &s, &pubkeys).unwrap();
            signing::verify_spend_auth(&ak, &request, &sig).unwrap();
        }
    }

    #[test]
    fn rejects_files_that_are_not_key_shares() {
        assert!(key_info_inner("deadbeef").is_err());
        assert!(commit_batch_inner("not hex", 1).is_err());
    }
}
