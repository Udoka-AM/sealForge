//! Trustee signing in the browser (`/trustee/{code}`).
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

/// Parse the alphas (`["<hex>", …]`) and both signers' commitments
/// (`{"<slot>": ["<hex>", …], …}`) a round 2 share is made over.
type Round2Inputs = (Vec<[u8; 32]>, BTreeMap<u16, Vec<Vec<u8>>>);

fn round2_inputs(alphas_json: &str, signer_commitments_json: &str) -> Res<Round2Inputs> {
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
    Ok((alphas, commitments))
}

/// Digest of the round 2 inputs (hex), the same as the CLI's: a chosen signer
/// signs it with its trustee key before signing, and checks its co-signer's
/// matches (`sealforge_api::auth::round2_inputs_digest`).
#[wasm_bindgen]
pub fn round2_inputs_digest(
    sighash_hex: &str,
    alphas_json: &str,
    signer_commitments_json: &str,
) -> Result<String, JsError> {
    js((|| {
        let (alphas, commitments) = round2_inputs(alphas_json, signer_commitments_json)?;
        Ok(hex::encode(sealforge_api::auth::round2_inputs_digest(
            &hex32(sighash_hex)?,
            &alphas,
            &commitments,
        )))
    })())
}

fn sign_batch_inner(
    key_package_hex: &str,
    sighash_hex: &str,
    alphas_json: &str,
    signer_commitments_json: &str,
    nonces_json: &str,
) -> Res<String> {
    let kp = key_package(key_package_hex)?;
    let (alphas, commitments) = round2_inputs(alphas_json, signer_commitments_json)?;
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
/// - `refund_chain_json`: refund batches only (empty otherwise), what this
///   page read through the trustee's own lightwallet server ([`RefundChain`])
///
/// Returns the [`sealforge_verify::VerifiedBatch`] as JSON, plus, for a refund
/// batch, `ledger_entry`: what to add to this trustee's ledger before any
/// share leaves the page.
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
    refund_chain_json: &str,
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
        refund_chain_json,
    ))
}

/// A bid transaction as read from the trustee's lightwallet server.
#[derive(serde::Deserialize)]
struct BidTransaction {
    /// Transaction id, internal byte order (hex), as in the published result.
    txid: String,
    /// The raw transaction (hex).
    raw: String,
    /// The height it was mined at.
    height: u32,
}

/// One entry of this trustee's ledger and whether its server has that transaction.
#[derive(serde::Deserialize)]
struct LedgerOnChain {
    entry: sealforge_verify::refunds::LedgerEntry,
    on_chain: bool,
}

/// What the page read from the chain to check a refund batch: every mined bid
/// transaction a note claims, the published result, this trustee's ledger,
/// and the server's tip.
#[derive(serde::Deserialize)]
struct RefundChain {
    tip: u32,
    bids: Vec<BidTransaction>,
    result: sealforge_api::types::PublishedResult,
    ledger: Vec<LedgerOnChain>,
}

#[derive(serde::Serialize)]
struct Checked {
    #[serde(flatten)]
    verified: sealforge_verify::VerifiedBatch,
    #[serde(skip_serializing_if = "Option::is_none")]
    ledger_entry: Option<sealforge_verify::refunds::LedgerEntry>,
}

/// Tie every note of a refund batch to its bid on chain: the same rules as
/// `sealforge trustee sign --lightwalletd` (sealforge_verify::refunds).
fn check_refunds(
    batch: &sealforge_api::types::BatchView,
    pczt: &[u8],
    fvk: &orchard::keys::FullViewingKey,
    network: zcash_protocol::consensus::NetworkType,
    verified: &sealforge_verify::VerifiedBatch,
    chain: RefundChain,
) -> Res<sealforge_verify::refunds::LedgerEntry> {
    use std::collections::BTreeMap;

    use sealforge_verify::refunds::{BidOnChain, RefundClaim, ResultBid};
    use zcash_protocol::consensus::NetworkType;

    let claims = batch
        .refunds
        .iter()
        .map(|n| {
            Ok(RefundClaim {
                bid_txid: hex32(&n.bid_txid)?,
                bid_output_index: n.bid_output_index,
                value_zat: n.value_zat,
            })
        })
        .collect::<Res<Vec<_>>>()?;
    let params = match network {
        NetworkType::Main => sealforge_scan::ChainParams::Mainnet,
        NetworkType::Test => sealforge_scan::ChainParams::Testnet,
        NetworkType::Regtest => sealforge_scan::ChainParams::regtest(),
    };
    let scanner = sealforge_scan::EscrowScanner::from_fvk(params, fvk);
    let mut bids = BTreeMap::new();
    for tx in &chain.bids {
        let txid = hex32(&tx.txid)?;
        let raw = hex::decode(&tx.raw).map_err(err)?;
        let payments = scanner
            .decrypt_transaction(&raw, tx.height)
            .map_err(|e| format!("decrypting bid {}: {e}", tx.txid))?;
        for p in payments.into_iter().filter(|p| p.txid == txid) {
            // Only bids a note in this batch claims.
            if claims.iter().any(|c| c.bid() == (p.txid, p.output_index)) {
                bids.insert(
                    (p.txid, p.output_index),
                    BidOnChain::from_payment(p.amount_zat, p.memo.as_deref()),
                );
            }
        }
    }
    let listed = chain
        .result
        .bids
        .iter()
        .map(|b| {
            Ok((
                (hex32(&b.txid)?, b.output_index),
                ResultBid {
                    commitment: hex32(&b.commitment)?,
                    refund_due_zat: b.refund_due_zat,
                },
            ))
        })
        .collect::<Res<BTreeMap<_, _>>>()?;
    let ledger: Vec<_> = chain
        .ledger
        .into_iter()
        .map(|l| (l.entry, l.on_chain))
        .collect();
    sealforge_verify::refunds::check_refund_batch(
        pczt, verified, &claims, &bids, &listed, &ledger, chain.tip,
    )
    .map_err(|r| {
        format!(
            "REFUSED ({}): {}. Don't sign; tell the organiser and the other trustees.",
            r.class, r.message
        )
    })
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
    refund_chain_json: &str,
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
    // Refund notes are tied to their bidders through the chain (security
    // review D1, second part): no chain reads, no refund signature.
    let chain: Option<RefundChain> = if refund_chain_json.trim().is_empty() {
        None
    } else {
        Some(serde_json::from_str(refund_chain_json).map_err(err)?)
    };
    if kind == BatchKind::Refund && chain.is_none() {
        return Err(
            "REFUSED: refund batches are checked against the chain. Set a lightwallet server you trust on this page first."
                .into(),
        );
    }
    let refuse = |r: sealforge_verify::Refusal| {
        format!(
            "REFUSED ({}): {}. Don't sign; tell the organiser and the other trustees.",
            r.class, r.message
        )
    };
    // The protocol fee (issue #74), from the published result the page read
    // with the chain context. Without it, a payout with a fee output is refused.
    let (fee, fee_already_signed) = match (&chain, kind) {
        (Some(c), BatchKind::Payout) => {
            let fee = sealforge_verify::fee_rule(
                c.result.fee_bps,
                c.result.fee_ua.as_deref(),
                c.result.fee_zat,
                c.result.bids.iter().map(|b| b.price_paid_zat),
            )
            .map_err(refuse)?;
            let already = fee.is_some() && {
                let ledger: Vec<_> = c
                    .ledger
                    .iter()
                    .map(|l| (l.entry.clone(), l.on_chain))
                    .collect();
                let txid = sealforge_verify::batch_txid(&pczt).map_err(refuse)?;
                sealforge_verify::refunds::fee_signed_before(&ledger, c.tip, &txid)
            };
            (fee, already)
        }
        _ => (None, false),
    };
    let rules = BatchRules {
        kind,
        refund_total_due_zat: (refund_total_due_zat >= 0.0).then_some(refund_total_due_zat as u64),
        payout_ua: batch.payout_ua.clone(),
        fee,
        fee_already_signed,
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
    .map_err(refuse)?;
    let ledger_entry = match (chain, kind) {
        (Some(chain), BatchKind::Refund) => Some(check_refunds(
            &batch, &pczt, &fvk, network, &verified, chain,
        )?),
        // A payout that pays the fee goes in the ledger too.
        _ if verified.protocol_fee_zat > 0 => Some(sealforge_verify::refunds::LedgerEntry {
            batch_txid: sealforge_verify::batch_txid(&pczt).map_err(refuse)?,
            expiry_height: verified.expiry_height,
            notes: Vec::new(),
            fee_zat: verified.protocol_fee_zat,
        }),
        _ => None,
    };
    serde_json::to_string(&Checked {
        verified,
        ledger_entry,
    })
    .map_err(err)
}
