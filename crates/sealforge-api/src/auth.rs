//! `SF-Sig` request signing (docs/05-BACKEND-SCHEMA.md §5.5).
//!
//! Organisers and trustees sign every API request with an Ed25519 key that
//! never leaves their machine. The server stores only public keys.
//!
//! ```text
//! Authorization: SF-Sig keyId=<b64 pubkey>, ts=<unix secs>, nonce=<b64 16 bytes>, sig=<b64 signature>
//!
//! signed message =
//!   "SF-Sig-v1\n" + METHOD + "\n" + path_and_query + "\n" + ts + "\n" + nonce_b64 + "\n" + hex(sha256(body))
//! ```
//!
//! The server rejects timestamps more than [`MAX_CLOCK_SKEW_SECS`] away from
//! its clock and any nonce it has already seen for that key.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SCHEME: &str = "SF-Sig";
pub const MAX_CLOCK_SKEW_SECS: i64 = 60;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("missing or malformed SF-Sig authorization header")]
    Malformed,
    #[error("request timestamp is outside the allowed clock skew")]
    StaleTimestamp,
    #[error("signature does not verify")]
    BadSignature,
}

/// A parsed `Authorization: SF-Sig …` header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedHeader {
    pub key: [u8; 32],
    pub ts: i64,
    pub nonce: [u8; 16],
    pub sig: [u8; 64],
}

/// The exact bytes that get signed.
pub fn canonical_message(
    method: &str,
    path_and_query: &str,
    ts: i64,
    nonce: &[u8; 16],
    body: &[u8],
) -> Vec<u8> {
    format!(
        "SF-Sig-v1\n{}\n{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path_and_query,
        ts,
        B64.encode(nonce),
        hex::encode(Sha256::digest(body)),
    )
    .into_bytes()
}

/// Client side: build the header value for a request.
pub fn sign_request(
    key: &SigningKey,
    method: &str,
    path_and_query: &str,
    ts: i64,
    nonce: &[u8; 16],
    body: &[u8],
) -> String {
    let sig = key.sign(&canonical_message(method, path_and_query, ts, nonce, body));
    format!(
        "{SCHEME} keyId={}, ts={ts}, nonce={}, sig={}",
        B64.encode(key.verifying_key().as_bytes()),
        B64.encode(nonce),
        B64.encode(sig.to_bytes()),
    )
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], AuthError> {
    B64.decode(value)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(AuthError::Malformed)
}

/// Parse an `Authorization` header value.
pub fn parse_header(value: &str) -> Result<SignedHeader, AuthError> {
    let params = value
        .strip_prefix(SCHEME)
        .and_then(|rest| rest.strip_prefix(' '))
        .ok_or(AuthError::Malformed)?;
    let (mut key, mut ts, mut nonce, mut sig) = (None, None, None, None);
    for part in params.split(',') {
        let (name, val) = part.trim().split_once('=').ok_or(AuthError::Malformed)?;
        match name {
            "keyId" => key = Some(decode_fixed::<32>(val)?),
            "ts" => ts = Some(val.parse::<i64>().map_err(|_| AuthError::Malformed)?),
            "nonce" => nonce = Some(decode_fixed::<16>(val)?),
            "sig" => sig = Some(decode_fixed::<64>(val)?),
            _ => return Err(AuthError::Malformed),
        }
    }
    Ok(SignedHeader {
        key: key.ok_or(AuthError::Malformed)?,
        ts: ts.ok_or(AuthError::Malformed)?,
        nonce: nonce.ok_or(AuthError::Malformed)?,
        sig: sig.ok_or(AuthError::Malformed)?,
    })
}

/// Server side: check the clock skew and the signature. Nonce replay is
/// checked separately against the database.
pub fn verify_request(
    header: &SignedHeader,
    method: &str,
    path_and_query: &str,
    body: &[u8],
    now: i64,
) -> Result<(), AuthError> {
    if (now - header.ts).abs() > MAX_CLOCK_SKEW_SECS {
        return Err(AuthError::StaleTimestamp);
    }
    let key = VerifyingKey::from_bytes(&header.key).map_err(|_| AuthError::BadSignature)?;
    let message = canonical_message(method, path_and_query, header.ts, &header.nonce, body);
    key.verify(&message, &Signature::from_bytes(&header.sig))
        .map_err(|_| AuthError::BadSignature)
}

/// Bytes a trustee signs for each DKG package, so trustees can check that
/// packages relayed by the coordinator really came from the sending trustee.
pub fn dkg_package_message(
    auction_code: &str,
    round: u8,
    from_slot: u16,
    to_slot: Option<u16>,
    payload: &[u8],
) -> Vec<u8> {
    format!(
        "sealforge-dkg-v1\n{auction_code}\n{round}\n{from_slot}\n{}\n{}",
        to_slot.map(|s| s.to_string()).unwrap_or_else(|| "*".into()),
        hex::encode(Sha256::digest(payload)),
    )
    .into_bytes()
}

/// Verify a trustee's DKG package signature.
pub fn verify_dkg_package(
    pubkey: &[u8; 32],
    signature: &[u8; 64],
    auction_code: &str,
    round: u8,
    from_slot: u16,
    to_slot: Option<u16>,
    payload: &[u8],
) -> bool {
    VerifyingKey::from_bytes(pubkey)
        .and_then(|key| {
            key.verify(
                &dkg_package_message(auction_code, round, from_slot, to_slot, payload),
                &Signature::from_bytes(signature),
            )
        })
        .is_ok()
}

/// Bytes a trustee signs to attest that it rebuilt an auction's escrow address
/// from its own copy of the FROST group key. Anyone holding the trustee's
/// public key can check the attestation without trusting the coordinator.
pub fn escrow_attestation_message(auction_code: &str, slot: u16, escrow_ua: &str) -> Vec<u8> {
    format!("sealforge-escrow-v1\n{auction_code}\n{slot}\n{escrow_ua}").into_bytes()
}

/// Sign an escrow attestation as a trustee.
pub fn sign_escrow_attestation(
    key: &SigningKey,
    auction_code: &str,
    slot: u16,
    escrow_ua: &str,
) -> [u8; 64] {
    key.sign(&escrow_attestation_message(auction_code, slot, escrow_ua))
        .to_bytes()
}

/// Verify a trustee's escrow attestation.
pub fn verify_escrow_attestation(
    pubkey: &[u8; 32],
    signature: &[u8; 64],
    auction_code: &str,
    slot: u16,
    escrow_ua: &str,
) -> bool {
    VerifyingKey::from_bytes(pubkey)
        .and_then(|key| {
            key.verify(
                &escrow_attestation_message(auction_code, slot, escrow_ua),
                &Signature::from_bytes(signature),
            )
        })
        .is_ok()
}

/// Sign a DKG package as the sending trustee.
pub fn sign_dkg_package(
    key: &SigningKey,
    auction_code: &str,
    round: u8,
    from_slot: u16,
    to_slot: Option<u16>,
    payload: &[u8],
) -> [u8; 64] {
    key.sign(&dkg_package_message(
        auction_code,
        round,
        from_slot,
        to_slot,
        payload,
    ))
    .to_bytes()
}

/// Bytes a trustee's long-term identity key signs to vouch for its key in one
/// auction: the auction, the slot, the per-auction key, and where the trustee
/// publishes its long-term key (a URL or handle). Bidders check the long-term
/// key against that place; the coordinator can't change any of it.
pub fn trustee_cert_message(
    auction_code: &str,
    slot: u16,
    auction_pubkey: &[u8; 32],
    published_at: &str,
) -> Vec<u8> {
    format!(
        "sealforge-trustee-cert-v1\n{auction_code}\n{slot}\n{}\n{published_at}",
        hex::encode(auction_pubkey)
    )
    .into_bytes()
}

/// Sign a trustee certificate with the trustee's long-term identity key.
pub fn sign_trustee_cert(
    identity_key: &SigningKey,
    auction_code: &str,
    slot: u16,
    auction_pubkey: &[u8; 32],
    published_at: &str,
) -> [u8; 64] {
    identity_key
        .sign(&trustee_cert_message(
            auction_code,
            slot,
            auction_pubkey,
            published_at,
        ))
        .to_bytes()
}

/// Verify a trustee certificate under the trustee's long-term identity key.
pub fn verify_trustee_cert(
    identity_pubkey: &[u8; 32],
    signature: &[u8; 64],
    auction_code: &str,
    slot: u16,
    auction_pubkey: &[u8; 32],
    published_at: &str,
) -> bool {
    VerifyingKey::from_bytes(identity_pubkey)
        .and_then(|key| {
            key.verify(
                &trustee_cert_message(auction_code, slot, auction_pubkey, published_at),
                &Signature::from_bytes(signature),
            )
        })
        .is_ok()
}

/// Where a trustee says it publishes its long-term key: a URL or a handle,
/// 1 to 200 printable characters with no spaces.
pub fn valid_published_at(value: &str) -> bool {
    (1..=200).contains(&value.len()) && value.chars().all(|c| c.is_ascii_graphic())
}

/// Digest of everything a round 2 FROST share depends on besides the key and
/// the nonces: the sighash, each spend's randomizer α, and every chosen
/// signer's round 1 commitments (slot order, one per spend). Both signers must
/// compute the same digest; a different one means the coordinator showed them
/// different commitment sets.
pub fn round2_inputs_digest(
    sighash: &[u8; 32],
    alphas: &[[u8; 32]],
    commitments: &std::collections::BTreeMap<u16, Vec<Vec<u8>>>,
) -> [u8; 32] {
    fn len(h: &mut Sha256, n: usize) {
        h.update((n as u64).to_le_bytes());
    }
    let mut h = Sha256::new();
    h.update(b"sealforge/round2-inputs/v1");
    h.update(sighash);
    len(&mut h, alphas.len());
    for alpha in alphas {
        h.update(alpha);
    }
    len(&mut h, commitments.len());
    for (slot, values) in commitments {
        h.update(slot.to_le_bytes());
        len(&mut h, values.len());
        for v in values {
            len(&mut h, v.len());
            h.update(v);
        }
    }
    h.finalize().into()
}

/// Bytes a chosen signer signs to state which round 2 inputs it will sign
/// over. Its co-signer checks this against its own digest, with the key from
/// the signer's public escrow attestation, before releasing any share.
pub fn round2_inputs_message(
    auction_code: &str,
    batch_id: &str,
    slot: u16,
    digest: &[u8; 32],
) -> Vec<u8> {
    format!(
        "sealforge-round2-inputs-v1\n{auction_code}\n{batch_id}\n{slot}\n{}",
        hex::encode(digest)
    )
    .into_bytes()
}

/// Sign a round 2 inputs statement as a chosen signer.
pub fn sign_round2_inputs(
    key: &SigningKey,
    auction_code: &str,
    batch_id: &str,
    slot: u16,
    digest: &[u8; 32],
) -> [u8; 64] {
    key.sign(&round2_inputs_message(auction_code, batch_id, slot, digest))
        .to_bytes()
}

/// Verify a signer's round 2 inputs statement.
pub fn verify_round2_inputs(
    pubkey: &[u8; 32],
    signature: &[u8; 64],
    auction_code: &str,
    batch_id: &str,
    slot: u16,
    digest: &[u8; 32],
) -> bool {
    VerifyingKey::from_bytes(pubkey)
        .and_then(|key| {
            key.verify(
                &round2_inputs_message(auction_code, batch_id, slot, digest),
                &Signature::from_bytes(signature),
            )
        })
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trustee_certs_bind_auction_slot_key_and_place() {
        let identity = SigningKey::from_bytes(&[9; 32]);
        let id_pub = identity.verifying_key().to_bytes();
        let auction_key = key().verifying_key().to_bytes();
        let url = "https://example.org/trustee";
        let sig = sign_trustee_cert(&identity, "CODE", 2, &auction_key, url);
        assert!(verify_trustee_cert(
            &id_pub,
            &sig,
            "CODE",
            2,
            &auction_key,
            url
        ));
        assert!(!verify_trustee_cert(
            &id_pub,
            &sig,
            "OTHER",
            2,
            &auction_key,
            url
        ));
        assert!(!verify_trustee_cert(
            &id_pub,
            &sig,
            "CODE",
            1,
            &auction_key,
            url
        ));
        assert!(!verify_trustee_cert(
            &id_pub, &sig, "CODE", 2, &[1; 32], url
        ));
        assert!(!verify_trustee_cert(
            &id_pub,
            &sig,
            "CODE",
            2,
            &auction_key,
            "@someone"
        ));
        assert!(valid_published_at(url) && valid_published_at("@alice"));
        assert!(!valid_published_at("") && !valid_published_at("a b"));
        assert!(!valid_published_at(&"a".repeat(201)) && !valid_published_at("line\nbreak"));
    }

    #[test]
    fn round2_inputs_bind_every_commitment_and_the_signer() {
        let mut set = std::collections::BTreeMap::new();
        set.insert(1u16, vec![vec![1u8; 64], vec![2; 64]]);
        set.insert(2u16, vec![vec![3u8; 64], vec![4; 64]]);
        let alphas = [[9u8; 32], [8; 32]];
        let digest = round2_inputs_digest(&[5; 32], &alphas, &set);
        let mut other = set.clone();
        other.get_mut(&2).unwrap()[1][0] ^= 1;
        assert_ne!(digest, round2_inputs_digest(&[5; 32], &alphas, &other));
        assert_ne!(digest, round2_inputs_digest(&[6; 32], &alphas, &set));

        let sig = sign_round2_inputs(&key(), "CODE", "batch", 1, &digest);
        let pk = key().verifying_key().to_bytes();
        assert!(verify_round2_inputs(&pk, &sig, "CODE", "batch", 1, &digest));
        // Not valid for another slot, batch or digest.
        assert!(!verify_round2_inputs(
            &pk, &sig, "CODE", "batch", 2, &digest
        ));
        assert!(!verify_round2_inputs(
            &pk, &sig, "CODE", "other", 1, &digest
        ));
        let changed = round2_inputs_digest(&[5; 32], &alphas, &other);
        assert!(!verify_round2_inputs(
            &pk, &sig, "CODE", "batch", 1, &changed
        ));
    }

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    #[test]
    fn round_trip() {
        let header = sign_request(&key(), "post", "/v1/auctions", 1_000, &[1; 16], b"{}");
        let parsed = parse_header(&header).unwrap();
        assert_eq!(parsed.key, *key().verifying_key().as_bytes());
        verify_request(&parsed, "POST", "/v1/auctions", b"{}", 1_030).unwrap();
    }

    #[test]
    fn rejects_tampering_and_stale_requests() {
        let parsed = parse_header(&sign_request(
            &key(),
            "POST",
            "/v1/auctions",
            1_000,
            &[1; 16],
            b"{}",
        ))
        .unwrap();
        assert_eq!(
            verify_request(&parsed, "POST", "/v1/auctions", b"{\"x\":1}", 1_000),
            Err(AuthError::BadSignature),
            "body changed"
        );
        assert_eq!(
            verify_request(&parsed, "POST", "/v1/auctions/other", b"{}", 1_000),
            Err(AuthError::BadSignature),
            "path changed"
        );
        assert_eq!(
            verify_request(&parsed, "PATCH", "/v1/auctions", b"{}", 1_000),
            Err(AuthError::BadSignature),
            "method changed"
        );
        assert_eq!(
            verify_request(&parsed, "POST", "/v1/auctions", b"{}", 1_061),
            Err(AuthError::StaleTimestamp)
        );
        let mut other_nonce = parsed.clone();
        other_nonce.nonce = [2; 16];
        assert_eq!(
            verify_request(&other_nonce, "POST", "/v1/auctions", b"{}", 1_000),
            Err(AuthError::BadSignature),
            "nonce is signed"
        );
    }

    #[test]
    fn rejects_malformed_headers() {
        for bad in [
            "",
            "Bearer abc",
            "SF-Sig keyId=xx",
            "SF-Sig keyId=AAAA, ts=1, nonce=AAAA, sig=AAAA, extra=1",
        ] {
            assert_eq!(parse_header(bad), Err(AuthError::Malformed), "{bad}");
        }
    }
}
