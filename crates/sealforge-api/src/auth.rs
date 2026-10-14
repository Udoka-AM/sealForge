//! `SF-Sig` request signing (docs/05-BACKEND-SCHEMA.md §5.5, task 2.2).
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

#[cfg(test)]
mod tests {
    use super::*;

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
