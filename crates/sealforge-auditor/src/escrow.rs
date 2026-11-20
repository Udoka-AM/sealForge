//! Escrow attestations (security review F10).
//!
//! Each trustee rebuilds the escrow address from its own copy of the FROST
//! group key and signs it with its Ed25519 key for the auction. [`check_escrow`]
//! lets a bidder check those signatures without trusting the coordinator:
//! every trustee listed must have signed exactly the address being shown.
//!
//! What this does not prove on its own: that the listed keys belong to the
//! real trustees. The coordinator publishes the list, so a coordinator that
//! invented all three trustees could sign for them. Pin the keys you got from
//! the trustees themselves (`pinned`), or rely on each trustee's CLI checking
//! that the public page lists its own key and the address it signed.

use sealforge_api::{
    auth::verify_escrow_attestation,
    types::{AuctionView, PublishedResult},
};
use serde::{Deserialize, Serialize};

/// One trustee as published: slot, label, key and signature (hex).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrusteeClaim {
    pub slot: u16,
    pub label: String,
    pub pubkey: Option<String>,
    pub signature: Option<String>,
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
                (_, Some(key), _) if pin.is_some_and(|(_, p)| !p.eq_ignore_ascii_case(key)) => {
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
            TrusteeCheck {
                slot: t.slot,
                label: t.label.clone(),
                pubkey: t.pubkey.clone(),
                verdict,
            }
        })
        .collect();
    let signed = checks
        .iter()
        .filter(|c| c.verdict == TrusteeVerdict::Signed)
        .count() as u32;
    // A pinned slot that isn't listed at all is a substitution too.
    let pins_listed = pinned
        .iter()
        .all(|(slot, _)| trustees.iter().any(|t| t.slot == *slot));
    let verdict = if escrow_ua.is_some()
        && !checks.is_empty()
        && pins_listed
        && checks.iter().all(|c| c.verdict == TrusteeVerdict::Signed)
    {
        EscrowVerdict::Verified
    } else if pins_listed && checks.iter().all(|c| c.verdict == TrusteeVerdict::Missing) {
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
    }
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
        })
        .collect();
    check_escrow(&view.code, view.escrow_ua.as_deref(), &trustees, pinned)
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
        })
        .collect();
    check_escrow(
        &published.auction_code,
        published.escrow_ua.as_deref(),
        &trustees,
        pinned,
    )
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
        }
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
            check_escrow(CODE, Some(UA), &[], &[]).verdict,
            EscrowVerdict::Unattested
        );
        assert_eq!(
            check_escrow(CODE, None, &all_signed(), &[]).verdict,
            EscrowVerdict::Unattested
        );
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
