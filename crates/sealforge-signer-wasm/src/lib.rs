//! Trustee signing in the browser (`/trustee/{code}`, task 3.10, S16).
//!
//! The same batch helpers the CLI uses (`sealforge_escrow::signing`), exposed
//! with hex/JSON strings. The page keeps the key package and nonces in memory
//! only: nothing here touches storage or the network.
//!
//! [`verify_batch`] runs the same transaction checks as `sealforge trustee
//! sign` (security review D1) before the page commits or signs.

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

/// Decode and check a batch's transaction before taking part in signing it
/// (security review D1): the shared co-signer checks (frost-zec spec § 6) and
/// sealForge's rule that every payment is owed.
///
/// The escrow's viewing key is rebuilt here from this trustee's own key share
/// (`ak`) and the coordinator's sealed `nk ‖ rivk`, and must give exactly
/// `escrow_ua`: the address on the public page, which the caller has already
/// checked carries this trustee's own valid attestation.
///
/// - `network`: "mainnet", "testnet" or "regtest"
/// - `batch_json`: the trustee's `BatchView`
/// - `refund_total_due_zat`: refunds owed in total by the published result
///   (refund batches), or a negative number when there is none
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn verify_batch(
    key_package_hex: &str,
    enc_secret_hex: &str,
    sealed_view_secret_hex: &str,
    escrow_ua: &str,
    network: &str,
    now_secs: f64,
    batch_json: &str,
    refund_total_due_zat: f64,
) -> Result<String, JsError> {
    js(verify_batch_inner(
        key_package_hex,
        enc_secret_hex,
        sealed_view_secret_hex,
        escrow_ua,
        network,
        now_secs,
        batch_json,
        refund_total_due_zat,
    ))
}

#[allow(clippy::too_many_arguments)]
fn verify_batch_inner(
    key_package_hex: &str,
    enc_secret_hex: &str,
    sealed_view_secret_hex: &str,
    escrow_ua: &str,
    network: &str,
    now_secs: f64,
    batch_json: &str,
    refund_total_due_zat: f64,
) -> Res<String> {
    use sealforge_verify::{frost_zec::keys, BatchKind, BatchRules};
    use zcash_protocol::consensus::NetworkType;

    let network = match network {
        "mainnet" => NetworkType::Main,
        "testnet" => NetworkType::Test,
        "regtest" => NetworkType::Regtest,
        other => return Err(format!("unknown network {other}")),
    };
    let kp = key_package(key_package_hex)?;
    let ak: [u8; 32] = kp
        .verifying_key()
        .serialize()
        .map_err(err)?
        .try_into()
        .map_err(|_| "unexpected group key".to_string())?;
    let enc_secret = crypto_box::SecretKey::from(hex32(enc_secret_hex)?);
    let view_secret: [u8; 64] = enc_secret
        .unseal(&hex::decode(sealed_view_secret_hex).map_err(err)?)
        .map_err(|_| "could not open the escrow view secret".to_string())?
        .try_into()
        .map_err(|_| "the escrow view secret has the wrong length".to_string())?;
    // The escrow FVK is ak ‖ nk ‖ rivk (sealforge_escrow::address).
    let mut fvk_bytes = [0u8; 96];
    fvk_bytes[..32].copy_from_slice(&ak);
    fvk_bytes[32..].copy_from_slice(&view_secret);
    let fvk = orchard::keys::FullViewingKey::from_bytes(&fvk_bytes)
        .ok_or("the escrow viewing key is invalid")?;
    let shown = keys::encode_address(
        &fvk.address_at(0u32, orchard::keys::Scope::External),
        network,
    )
    .map_err(err)?;
    if shown != escrow_ua {
        return Err(
            "ESCROW KEY MISMATCH: the viewing key the coordinator sent doesn't give the escrow address you attested. Don't sign."
                .into(),
        );
    }

    let batch: sealforge_api::types::BatchView = serde_json::from_str(batch_json).map_err(err)?;
    let pczt = hex::decode(
        batch
            .pczt
            .as_deref()
            .ok_or("REFUSED: the coordinator sent no transaction to check")?,
    )
    .map_err(err)?;
    let kind = BatchKind::parse(&batch.kind).ok_or("REFUSED: unknown batch kind")?;
    // Refund notes are tied to their bidders by reading each bid from the
    // chain, which this page doesn't do yet (security review D1, second part).
    if kind == BatchKind::Refund {
        return Err(
            "REFUSED: refund batches are checked against the chain, which this page can't do yet. Sign them with `sealforge trustee sign --lightwalletd <server you trust>`."
                .into(),
        );
    }
    let rules = BatchRules {
        kind,
        refund_total_due_zat: (refund_total_due_zat >= 0.0).then_some(refund_total_due_zat as u64),
        payout_ua: batch.payout_ua.clone(),
    };
    let spends = batch
        .spends
        .iter()
        .map(|s| Ok((s.action_index as usize, hex32(&s.alpha)?)))
        .collect::<Res<Vec<_>>>()?;
    let verified = sealforge_verify::verify_batch(
        &pczt,
        &fvk,
        network,
        now_secs as u64,
        &hex32(&batch.sighash)?,
        &spends,
        &rules,
    )
    .map_err(|r| {
        format!(
            "REFUSED ({}): {}. Don't sign; tell the organiser and the other trustees.",
            r.class, r.message
        )
    })?;
    serde_json::to_string(&verified).map_err(err)
}
