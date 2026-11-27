//! Escrow attestations (security review F10).
//!
//! Each trustee rebuilds the escrow address from its own copy of the FROST
//! group key and signs it with its Ed25519 key for the auction. [`check_escrow`]
//! lets a bidder check those signatures without trusting the coordinator:
//! every trustee listed must have signed exactly the address being shown.
//!
//! The check fails closed: it is only `Verified` when exactly the protocol's
//! trustee slots ([`TRUSTEE_SLOTS`]) are listed, each with its own key, and
//! every one signed this auction, slot and address. A list a coordinator padded,
//! trimmed or filled with one key is `Failed`, not "fewer trustees".
//!
//! What this does not prove on its own: that the listed keys belong to the
//! real trustees. The coordinator publishes the list, so a coordinator that
//! invented all three trustees could sign for them. Each trustee can certify
//! its key with a long-term key it publishes somewhere it controls (stream 4):
//! a certificate that doesn't verify, or one long-term key on two seats, fails
//! the check, and [`require_identities`] fails it unless the long-term keys
//! you know are among the certified ones. You can also pin per-auction keys
//! (`pinned`), and each trustee's CLI checks that the public page lists its
//! own key and the address it signed. None of this proves the trustees are
//! independent people: one party can publish three long-term keys too.

use std::collections::BTreeSet;

use sealforge_api::{
    auth::{verify_escrow_attestation, verify_trustee_cert},
    types::{AuctionView, Network, PublishedResult, TrusteeIdentityCert, TRUSTEE_SLOTS},
};
use serde::{Deserialize, Serialize};

/// One trustee as published: slot, label, key and signature (hex).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrusteeClaim {
    pub slot: u16,
    pub label: String,
    pub pubkey: Option<String>,
    pub signature: Option<String>,
    /// The trustee's long-term key certificate, if published.
    #[serde(default)]
    pub identity: Option<TrusteeIdentityCert>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityVerdict {
    /// A long-term key vouches for this trustee's key in this auction and slot.
    Certified,
    /// No certificate published: nothing ties this key to a known trustee.
    None,
    /// A certificate is published but doesn't verify.
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrusteeVerdict {
    /// The signature over this auction, slot and address checks out.
    Signed,
    /// No key or no signature published.
    Missing,
    /// A signature is published but does not verify for this address.
    Invalid,
    /// The key differs from the one you pinned for this slot.
    WrongKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrusteeCheck {
    pub slot: u16,
    pub label: String,
    pub pubkey: Option<String>,
    pub verdict: TrusteeVerdict,
    #[serde(default = "no_identity")]
    pub identity: IdentityVerdict,
    /// The certifying long-term key (hex) and where the trustee publishes it,
    /// when [`IdentityVerdict::Certified`]: compare them with that place.
    #[serde(default)]
    pub identity_pubkey: Option<String>,
    #[serde(default)]
    pub identity_published_at: Option<String>,
}

fn no_identity() -> IdentityVerdict {
    IdentityVerdict::None
}

fn check_identity(
    auction_code: &str,
    slot: u16,
    pubkey: Option<&str>,
    cert: Option<&TrusteeIdentityCert>,
) -> IdentityVerdict {
    let Some(cert) = cert else {
        return IdentityVerdict::None;
    };
    match (
        pubkey.and_then(hex_array::<32>),
        hex_array::<32>(&cert.pubkey),
        hex_array::<64>(&cert.signature),
    ) {
        (Some(key), Some(identity), Some(sig))
            if verify_trustee_cert(
                &identity,
                &sig,
                auction_code,
                slot,
                &key,
                &cert.published_at,
            ) =>
        {
            IdentityVerdict::Certified
        }
        _ => IdentityVerdict::Invalid,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscrowVerdict {
    /// Every trustee signed this address.
    Verified,
    /// Nothing was signed: an auction from before attestations, or one whose
    /// attestations are being withheld. Treat as unverified.
    Unattested,
    /// At least one signature is missing, invalid or from an unexpected key.
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowCheck {
    pub verdict: EscrowVerdict,
    pub escrow_ua: Option<String>,
    pub signed: u32,
    pub total: u32,
    pub trustees: Vec<TrusteeCheck>,
    /// Why the trustee list itself is unacceptable (wrong slots, a key used
    /// twice, a pinned slot missing). Empty when the list is well formed.
    #[serde(default)]
    pub problems: Vec<String>,
}

impl EscrowCheck {
    pub fn verified(&self) -> bool {
        self.verdict == EscrowVerdict::Verified
    }
}

fn hex_array<const N: usize>(value: &str) -> Option<[u8; N]> {
    hex::decode(value).ok()?.try_into().ok()
}

/// Check every trustee's attestation over `escrow_ua`. `pinned` holds
/// `(slot, pubkey hex)` pairs you got from the trustees directly; a listed key
/// that differs fails the check.
pub fn check_escrow(
    auction_code: &str,
    escrow_ua: Option<&str>,
    trustees: &[TrusteeClaim],
    pinned: &[(u16, String)],
) -> EscrowCheck {
    let checks: Vec<TrusteeCheck> = trustees
        .iter()
        .map(|t| {
            let pin = pinned.iter().find(|(slot, _)| *slot == t.slot);
            let verdict = match (escrow_ua, t.pubkey.as_deref(), t.signature.as_deref()) {
                // A pinned slot must carry exactly the pinned key. Publishing no
                // key at all is a substitution too, not "unattested".
                (_, key, _)
                    if pin.is_some_and(|(_, p)| key.is_none_or(|k| !p.eq_ignore_ascii_case(k))) =>
                {
                    TrusteeVerdict::WrongKey
                }
                (Some(ua), Some(key), Some(sig)) => {
                    match (hex_array::<32>(key), hex_array::<64>(sig)) {
                        (Some(key), Some(sig))
                            if verify_escrow_attestation(&key, &sig, auction_code, t.slot, ua) =>
                        {
                            TrusteeVerdict::Signed
                        }
                        _ => TrusteeVerdict::Invalid,
                    }
                }
                _ => TrusteeVerdict::Missing,
            };
            let identity = check_identity(
                auction_code,
                t.slot,
                t.pubkey.as_deref(),
                t.identity.as_ref(),
            );
            let certified = identity == IdentityVerdict::Certified;
            TrusteeCheck {
                slot: t.slot,
                label: t.label.clone(),
                pubkey: t.pubkey.clone(),
                verdict,
                identity,
                identity_pubkey: certified
                    .then(|| t.identity.as_ref().map(|c| c.pubkey.to_ascii_lowercase()))
                    .flatten(),
                identity_published_at: certified
                    .then(|| t.identity.as_ref().map(|c| c.published_at.clone()))
                    .flatten(),
            }
        })
        .collect();
    let signed = checks
        .iter()
        .filter(|c| c.verdict == TrusteeVerdict::Signed)
        .count() as u32;
    let mut problems = list_problems(trustees, pinned);
    for c in checks
        .iter()
        .filter(|c| c.identity == IdentityVerdict::Invalid)
    {
        problems.push(format!(
            "slot {}'s long-term key certificate doesn't verify",
            c.slot
        ));
    }
    let identities: Vec<&String> = checks
        .iter()
        .filter_map(|c| c.identity_pubkey.as_ref())
        .collect();
    if identities.iter().collect::<BTreeSet<_>>().len() != identities.len() {
        problems.push("one long-term key vouches for more than one trustee seat".to_owned());
    }
    let verdict = if !problems.is_empty() {
        EscrowVerdict::Failed
    } else if escrow_ua.is_some() && checks.iter().all(|c| c.verdict == TrusteeVerdict::Signed) {
        EscrowVerdict::Verified
    } else if checks.iter().all(|c| c.verdict == TrusteeVerdict::Missing) {
        EscrowVerdict::Unattested
    } else {
        EscrowVerdict::Failed
    };
    EscrowCheck {
        verdict,
        escrow_ua: escrow_ua.map(str::to_owned),
        signed,
        total: checks.len() as u32,
        trustees: checks,
        problems,
    }
}

/// Fail `check` unless every long-term key in `expected` (hex, e.g. from the
/// trustees' own websites) certifies one of its trustees.
pub fn require_identities(check: &mut EscrowCheck, expected: &[String]) {
    for key in expected {
        let key = key.trim().to_ascii_lowercase();
        if !check
            .trustees
            .iter()
            .any(|t| t.identity_pubkey.as_deref() == Some(key.as_str()))
        {
            check.problems.push(format!(
                "long-term key {} doesn't certify any trustee of this auction",
                &key[..key.len().min(16)]
            ));
            check.verdict = EscrowVerdict::Failed;
        }
    }
}

/// On mainnet every trustee must be vouched for by a long-term key (#67):
/// without one, nothing ties a trustee seat to a person, so one party could
/// hold all three. Fails `check` when any seat lacks a valid certificate.
pub fn require_certified_on_mainnet(check: &mut EscrowCheck, network: Network) {
    if network != Network::Mainnet {
        return;
    }
    for t in &check.trustees {
        if t.identity != IdentityVerdict::Certified {
            check.problems.push(format!(
                "slot {} has no valid long-term key; on mainnet every trustee must have one",
                t.slot
            ));
            check.verdict = EscrowVerdict::Failed;
        }
    }
}

/// What is wrong with the trustee list itself, before any signature counts.
fn list_problems(trustees: &[TrusteeClaim], pinned: &[(u16, String)]) -> Vec<String> {
    let mut problems = Vec::new();
    let slots: Vec<u16> = trustees.iter().map(|t| t.slot).collect();
    let distinct: BTreeSet<u16> = slots.iter().copied().collect();
    let expected = BTreeSet::from(TRUSTEE_SLOTS);
    if slots.len() != expected.len() || distinct != expected {
        problems.push(format!(
            "expected trustee slots {:?}, the page lists {slots:?}",
            TRUSTEE_SLOTS
        ));
    }
    let keys: Vec<String> = trustees
        .iter()
        .filter_map(|t| t.pubkey.as_deref())
        .map(str::to_ascii_lowercase)
        .collect();
    let distinct_keys: BTreeSet<&String> = keys.iter().collect();
    if distinct_keys.len() != keys.len() {
        problems.push("the same key is listed for more than one trustee".to_owned());
    }
    // A pinned slot that isn't listed at all is a substitution too.
    for (slot, _) in pinned {
        if !trustees.iter().any(|t| t.slot == *slot) {
            problems.push(format!("pinned slot {slot} is not listed"));
        }
    }
    problems
}

/// [`check_escrow`] over the auction page's data, before you bid.
pub fn check_escrow_view(view: &AuctionView, pinned: &[(u16, String)]) -> EscrowCheck {
    let trustees: Vec<TrusteeClaim> = view
        .trustees
        .iter()
        .map(|t| TrusteeClaim {
            slot: t.slot,
            label: t.label.clone(),
            pubkey: t.pubkey.clone(),
            signature: t.escrow_attestation.clone(),
            identity: t.identity.clone(),
        })
        .collect();
    let mut check = check_escrow(&view.code, view.escrow_ua.as_deref(), &trustees, pinned);
    require_certified_on_mainnet(&mut check, view.network);
    check
}

/// [`check_escrow`] over a published result, after close.
pub fn check_escrow_result(published: &PublishedResult, pinned: &[(u16, String)]) -> EscrowCheck {
    let trustees: Vec<TrusteeClaim> = published
        .trustees
        .iter()
        .map(|t| TrusteeClaim {
            slot: t.slot,
            label: t.label.clone(),
            pubkey: t.pubkey.clone(),
            signature: t.signature.clone(),
            identity: t.identity.clone(),
        })
        .collect();
    let mut check = check_escrow(
        &published.auction_code,
        published.escrow_ua.as_deref(),
        &trustees,
        pinned,
    );
    require_certified_on_mainnet(&mut check, published.network);
    check
}

/// [`check_escrow_view`] from the auction page's JSON. Extra fields are
/// ignored, so a coordinator can't make an open auction pass as a result.
pub fn check_view_json(json: &str, pinned: &[(u16, String)]) -> Result<EscrowCheck, String> {
    let view: AuctionView =
        serde_json::from_str(json).map_err(|e| format!("invalid auction view: {e}"))?;
    Ok(check_escrow_view(&view, pinned))
}

/// [`check_escrow_result`] from a published result's JSON.
pub fn check_result_json(json: &str, pinned: &[(u16, String)]) -> Result<EscrowCheck, String> {
    let published: PublishedResult =
        serde_json::from_str(json).map_err(|e| format!("invalid published result: {e}"))?;
    Ok(check_escrow_result(&published, pinned))
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use sealforge_api::auth::sign_escrow_attestation;

    use super::*;

    const CODE: &str = "7K3QX9MZ2P";
    const UA: &str = "utest1escrow";

    fn key(i: u8) -> SigningKey {
        SigningKey::from_bytes(&[i; 32])
    }

    fn claim(slot: u16, ua: &str) -> TrusteeClaim {
        let k = key(slot as u8);
        TrusteeClaim {
            slot,
            label: format!("T{slot}"),
            pubkey: Some(hex::encode(k.verifying_key().to_bytes())),
            signature: Some(hex::encode(sign_escrow_attestation(&k, CODE, slot, ua))),
            identity: None,
        }
    }

    /// `claim` plus a certificate from long-term key `who`.
    fn certified(slot: u16, who: u8) -> TrusteeClaim {
        let mut c = claim(slot, UA);
        let auction_key = key(slot as u8).verifying_key().to_bytes();
        let identity = SigningKey::from_bytes(&[100 + who; 32]);
        let place = format!("https://example.org/trustee-{who}");
        c.identity = Some(TrusteeIdentityCert {
            pubkey: hex::encode(identity.verifying_key().to_bytes()),
            signature: hex::encode(sealforge_api::auth::sign_trustee_cert(
                &identity,
                CODE,
                slot,
                &auction_key,
                &place,
            )),
            published_at: place,
        });
        c
    }

    fn identity_hex(who: u8) -> String {
        hex::encode(
            SigningKey::from_bytes(&[100 + who; 32])
                .verifying_key()
                .to_bytes(),
        )
    }

    #[test]
    fn certified_trustees_verify_and_show_their_long_term_keys() {
        let t: Vec<_> = (1..=3).map(|s| certified(s, s as u8)).collect();
        let mut check = check_escrow(CODE, Some(UA), &t, &[]);
        assert_eq!(check.verdict, EscrowVerdict::Verified, "{check:?}");
        assert!(check
            .trustees
            .iter()
            .all(|c| c.identity == IdentityVerdict::Certified));
        assert_eq!(check.trustees[1].identity_pubkey, Some(identity_hex(2)));
        assert_eq!(
            check.trustees[1].identity_published_at.as_deref(),
            Some("https://example.org/trustee-2")
        );
        // Pinning long-term keys you know: present passes, absent fails.
        require_identities(
            &mut check,
            &[identity_hex(1), identity_hex(3).to_uppercase()],
        );
        assert_eq!(check.verdict, EscrowVerdict::Verified);
        require_identities(&mut check, &[identity_hex(9)]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
    }

    #[test]
    fn uncertified_trustees_still_verify_but_show_no_identity() {
        let mut t = all_signed();
        t[0] = certified(1, 1);
        let mut check = check_escrow(CODE, Some(UA), &t, &[]);
        assert_eq!(check.verdict, EscrowVerdict::Verified);
        assert_eq!(check.trustees[1].identity, IdentityVerdict::None);
        assert_eq!(check.trustees[1].identity_pubkey, None);
        // ...but a known long-term key that isn't there fails.
        require_identities(&mut check, &[identity_hex(2)]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
    }

    #[test]
    fn mainnet_requires_every_trustee_to_be_certified() {
        let mut t = all_signed();
        t[0] = certified(1, 1);
        // Testnet: one certified trustee is enough to verify.
        let mut check = check_escrow(CODE, Some(UA), &t, &[]);
        require_certified_on_mainnet(&mut check, Network::Testnet);
        assert_eq!(check.verdict, EscrowVerdict::Verified);
        // Mainnet: the two uncertified seats fail it.
        require_certified_on_mainnet(&mut check, Network::Mainnet);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert_eq!(check.problems.len(), 2, "{:?}", check.problems);
        // All three certified: mainnet verifies.
        let t = vec![certified(1, 1), certified(2, 2), certified(3, 3)];
        let mut check = check_escrow(CODE, Some(UA), &t, &[]);
        require_certified_on_mainnet(&mut check, Network::Mainnet);
        assert_eq!(
            check.verdict,
            EscrowVerdict::Verified,
            "{:?}",
            check.problems
        );
    }

    #[test]
    fn a_bad_or_reused_certificate_fails() {
        // Moved to another slot: the certificate names slot 1.
        let mut t: Vec<_> = (1..=3).map(|s| certified(s, s as u8)).collect();
        t[1].identity = t[0].identity.clone();
        let check = check_escrow(CODE, Some(UA), &t, &[]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert_eq!(check.trustees[1].identity, IdentityVerdict::Invalid);
        // The published place edited after signing.
        let mut t: Vec<_> = (1..=3).map(|s| certified(s, s as u8)).collect();
        t[2].identity.as_mut().unwrap().published_at = "https://attacker.example".into();
        assert_eq!(
            check_escrow(CODE, Some(UA), &t, &[]).verdict,
            EscrowVerdict::Failed
        );
        // One long-term key vouching for two seats: one person, two votes.
        let t = vec![certified(1, 1), certified(2, 1), certified(3, 3)];
        let check = check_escrow(CODE, Some(UA), &t, &[]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert!(check
            .problems
            .iter()
            .any(|p| p.contains("more than one trustee seat")));
    }

    fn all_signed() -> Vec<TrusteeClaim> {
        (1..=3).map(|s| claim(s, UA)).collect()
    }

    #[test]
    fn three_valid_attestations_verify() {
        let check = check_escrow(CODE, Some(UA), &all_signed(), &[]);
        assert_eq!(check.verdict, EscrowVerdict::Verified);
        assert_eq!((check.signed, check.total), (3, 3));
    }

    #[test]
    fn a_swapped_address_fails_every_trustee() {
        let check = check_escrow(CODE, Some("utest1attacker"), &all_signed(), &[]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert!(check
            .trustees
            .iter()
            .all(|t| t.verdict == TrusteeVerdict::Invalid));
    }

    #[test]
    fn signatures_are_bound_to_the_auction_and_slot() {
        // Replayed from another auction.
        let check = check_escrow("OTHERCODE1", Some(UA), &all_signed(), &[]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        // Slots swapped between trustees.
        let mut t = all_signed();
        t[0].slot = 2;
        t[1].slot = 1;
        assert_eq!(
            check_escrow(CODE, Some(UA), &t, &[]).verdict,
            EscrowVerdict::Failed
        );
    }

    #[test]
    fn one_missing_signature_fails() {
        let mut t = all_signed();
        t[2].signature = None;
        let check = check_escrow(CODE, Some(UA), &t, &[]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert_eq!(check.trustees[2].verdict, TrusteeVerdict::Missing);
        assert_eq!(check.signed, 2);
    }

    #[test]
    fn nothing_signed_is_unattested_not_verified() {
        let t: Vec<_> = all_signed()
            .into_iter()
            .map(|mut c| {
                c.signature = None;
                c
            })
            .collect();
        assert_eq!(
            check_escrow(CODE, Some(UA), &t, &[]).verdict,
            EscrowVerdict::Unattested
        );
        assert_eq!(
            check_escrow(CODE, None, &all_signed(), &[]).verdict,
            EscrowVerdict::Unattested
        );
        // An empty list is not "nothing signed yet": the protocol always has
        // three trustees, so a page with none has been tampered with.
        assert_eq!(
            check_escrow(CODE, Some(UA), &[], &[]).verdict,
            EscrowVerdict::Failed
        );
    }

    #[test]
    fn one_invented_trustee_is_not_enough() {
        // Daniel's review: "All 1 trustees signed this address".
        let check = check_escrow(CODE, Some(UA), &[claim(1, UA)], &[]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert!(!check.problems.is_empty());
    }

    #[test]
    fn one_key_in_every_slot_fails() {
        let k = key(1);
        let t: Vec<_> = (1..=3)
            .map(|slot| TrusteeClaim {
                slot,
                label: format!("T{slot}"),
                pubkey: Some(hex::encode(k.verifying_key().to_bytes()).to_uppercase()),
                signature: Some(hex::encode(sign_escrow_attestation(&k, CODE, slot, UA))),
                identity: None,
            })
            .collect();
        // Every signature is individually valid, yet one party holds all seats.
        let check = check_escrow(CODE, Some(UA), &t, &[]);
        assert_eq!(check.signed, 3);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
    }

    #[test]
    fn slots_must_be_exactly_one_to_three() {
        for slots in [[1, 2, 2], [1, 2, 4], [0, 1, 2]] {
            let t: Vec<_> = slots.iter().map(|s| claim(*s, UA)).collect();
            assert_eq!(
                check_escrow(CODE, Some(UA), &t, &[]).verdict,
                EscrowVerdict::Failed,
                "{slots:?}"
            );
        }
        let mut four = all_signed();
        four.push(claim(4, UA));
        assert_eq!(
            check_escrow(CODE, Some(UA), &four, &[]).verdict,
            EscrowVerdict::Failed
        );
    }

    fn open_view(trustees: &[TrusteeClaim]) -> serde_json::Value {
        serde_json::json!({
            "code": CODE, "title": "Lot", "description": "", "lot_image_url": null,
            "network": "testnet", "mode": "uniform_price", "bid_mode": "deposit",
            "quantity": 1, "reserve_zat": 1_000_000, "open_height": 100,
            "close_height": 300, "status": "open", "escrow_ua": UA,
            "trustees": trustees.iter().map(|t| serde_json::json!({
                "slot": t.slot, "label": t.label, "dkg_status": "complete",
                "pubkey": t.pubkey, "escrow_attestation": t.signature,
            })).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn an_open_auction_dressed_as_a_result_is_still_checked_as_a_view() {
        // Daniel's review: adding `auction_code` and result fields to an open
        // auction once made the page treat it as an old, unsigned result.
        let unsigned: Vec<_> = all_signed()
            .into_iter()
            .map(|mut c| {
                c.signature = None;
                c
            })
            .collect();
        let mut view = open_view(&unsigned);
        let extra = serde_json::json!({
            "auction_code": CODE, "uivk": "uivktest1", "merkle_root": "00",
            "scan_from_height": 100, "scan_to_height": 300, "bids": [],
        });
        for (k, v) in extra.as_object().unwrap() {
            view[k] = v.clone();
        }
        let check = check_view_json(&view.to_string(), &[]).unwrap();
        assert!(!check.verified());
        // The view path never yields the result-only "older auction" reading:
        // with nothing signed it is unattested, which the page blocks on.
        assert_eq!(check.verdict, EscrowVerdict::Unattested);
        // And a real view parses and verifies through the same entry point.
        let check = check_view_json(&open_view(&all_signed()).to_string(), &[]).unwrap();
        assert!(check.verified(), "{check:?}");
        // A view isn't a result: the result entry point refuses it.
        assert!(check_result_json(&open_view(&all_signed()).to_string(), &[]).is_err());
    }

    #[test]
    fn a_pinned_slot_without_a_key_fails() {
        // Dropping the key (and signature) of a pinned slot must not turn into
        // the softer "unattested" verdict.
        let t: Vec<_> = all_signed()
            .into_iter()
            .map(|mut c| {
                c.signature = None;
                if c.slot == 2 {
                    c.pubkey = None;
                }
                c
            })
            .collect();
        let real = hex::encode(key(2).verifying_key().to_bytes());
        let check = check_escrow(CODE, Some(UA), &t, &[(2, real)]);
        assert_eq!(check.trustees[1].verdict, TrusteeVerdict::WrongKey);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
    }

    #[test]
    fn pinned_keys_catch_invented_trustees() {
        // The coordinator replaces trustee 2 with a key it controls and signs.
        let mut t = all_signed();
        let fake = key(99);
        t[1].pubkey = Some(hex::encode(fake.verifying_key().to_bytes()));
        t[1].signature = Some(hex::encode(sign_escrow_attestation(&fake, CODE, 2, UA)));
        // Without pins the forgery is self-consistent…
        assert!(check_escrow(CODE, Some(UA), &t, &[]).verified());
        // …with the real trustee's key pinned it fails.
        let real = hex::encode(key(2).verifying_key().to_bytes());
        let check = check_escrow(CODE, Some(UA), &t, &[(2, real.clone())]);
        assert_eq!(check.verdict, EscrowVerdict::Failed);
        assert_eq!(check.trustees[1].verdict, TrusteeVerdict::WrongKey);
        // A pinned slot that was dropped from the list fails too.
        let dropped: Vec<_> = all_signed().into_iter().filter(|c| c.slot != 2).collect();
        assert_eq!(
            check_escrow(CODE, Some(UA), &dropped, &[(2, real)]).verdict,
            EscrowVerdict::Failed
        );
    }
}
