//! Distributed key generation, run locally by each trustee.
//!
//! Round 1 packages are broadcast; round 2 packages are sent to one recipient
//! each and must be encrypted to that trustee before the coordinator relays them
//! (task 2.4). `finish` returns the trustee's key package (secret, stays on the
//! trustee's machine) and the public key package (shared with the coordinator).

use std::collections::BTreeMap;

use rand::{CryptoRng, RngCore};

use crate::{
    frost::{
        self,
        keys::{dkg, KeyPackage, PublicKeyPackage},
        Identifier,
    },
    EscrowError, Threshold,
};

/// FROST identifier for a trustee slot (1-based, as in `trustees.slot`).
pub fn identifier(slot: u16) -> Result<Identifier, EscrowError> {
    Ok(Identifier::try_from(slot)?)
}

/// Round 1: generate this trustee's secret and the package to broadcast.
pub fn round1<R: RngCore + CryptoRng>(
    slot: u16,
    threshold: Threshold,
    rng: &mut R,
) -> Result<(dkg::round1::SecretPackage, dkg::round1::Package), EscrowError> {
    Ok(dkg::part1(
        identifier(slot)?,
        threshold.max_signers,
        threshold.min_signers,
        rng,
    )?)
}

/// Round 2: consume the other trustees' round 1 packages and produce one
/// package per recipient.
pub fn round2(
    secret: dkg::round1::SecretPackage,
    received_round1: &BTreeMap<Identifier, dkg::round1::Package>,
) -> Result<
    (
        dkg::round2::SecretPackage,
        BTreeMap<Identifier, dkg::round2::Package>,
    ),
    EscrowError,
> {
    Ok(dkg::part2(secret, received_round1)?)
}

/// Round 3: derive this trustee's key package and the group's public key package.
/// The group key is normalised to even Y, as Orchard requires for `ak`.
pub fn finish(
    secret: &dkg::round2::SecretPackage,
    received_round1: &BTreeMap<Identifier, dkg::round1::Package>,
    received_round2: &BTreeMap<Identifier, dkg::round2::Package>,
) -> Result<(KeyPackage, PublicKeyPackage), EscrowError> {
    Ok(dkg::part3(secret, received_round1, received_round2)?)
}

/// Run all three rounds in one process. For tests and local demos only:
/// in production every trustee runs their rounds on their own machine.
pub fn run_local<R: RngCore + CryptoRng>(
    threshold: Threshold,
    rng: &mut R,
) -> Result<(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage), EscrowError> {
    let slots: Vec<u16> = (1..=threshold.max_signers).collect();

    let mut r1_secrets = BTreeMap::new();
    let mut r1_packages = BTreeMap::new();
    for &slot in &slots {
        let (secret, package) = round1(slot, threshold, rng)?;
        r1_secrets.insert(identifier(slot)?, secret);
        r1_packages.insert(identifier(slot)?, package);
    }

    let others = |me: &Identifier, all: &BTreeMap<Identifier, dkg::round1::Package>| {
        all.iter()
            .filter(|(id, _)| *id != me)
            .map(|(id, p)| (*id, p.clone()))
            .collect::<BTreeMap<_, _>>()
    };

    let mut r2_secrets = BTreeMap::new();
    let mut r2_outbox: BTreeMap<Identifier, BTreeMap<Identifier, dkg::round2::Package>> =
        BTreeMap::new();
    for (id, secret) in r1_secrets {
        let (secret2, outgoing) = round2(secret, &others(&id, &r1_packages))?;
        r2_secrets.insert(id, secret2);
        for (to, package) in outgoing {
            r2_outbox.entry(to).or_default().insert(id, package);
        }
    }

    let mut key_packages = BTreeMap::new();
    let mut public = None;
    for (id, secret2) in &r2_secrets {
        let (key_package, pubkeys) = finish(secret2, &others(id, &r1_packages), &r2_outbox[id])?;
        if let Some(existing) = &public {
            assert_eq!(existing, &pubkeys, "trustees disagree on the group key");
        }
        public = Some(pubkeys);
        key_packages.insert(*id, key_package);
    }
    Ok((key_packages, public.expect("at least one trustee")))
}

/// Serialized group verifying key (`ak` for the escrow), 32 bytes.
pub fn group_key_bytes(pubkeys: &PublicKeyPackage) -> Result<[u8; 32], EscrowError> {
    let bytes = pubkeys.verifying_key().serialize()?;
    bytes
        .try_into()
        .map_err(|_| EscrowError::Frost(frost::Error::MalformedVerifyingKey))
}
