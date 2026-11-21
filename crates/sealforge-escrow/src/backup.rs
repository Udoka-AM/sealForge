//! Trustee backup strings: everything one trustee needs to take part in
//! spending from an escrow without the coordinator, in one string to keep
//! offline.
//!
//! The shared FROST member backup draft (zcash/zips#1432, the draft ZIP that
//! grew out of the discussion in zcash/zips#1413; `zkfrost` version 1)
//! stores the group's shared Orchard spending key `sk` and recomputes `nk` and
//! `rivk` from it. A sealForge escrow has no `sk`: the coordinator samples
//! `nk` and `rivk` directly (see [`crate::address::escrow_fvk`]), so version 1
//! can't hold it. The draft lists these "groups with no `sk`" as an open issue
//! for a version 2 that would carry the raw 96-byte Orchard full viewing key
//! and a birthday height instead. Until that version is specified, this module
//! writes that content under its own prefix, `sfbackup`, so a sealForge string
//! is never mistaken for a `zkfrost` one. Such groups are not quantum
//! recoverable under ZIP 2005: there is no `sk` to recover them from.
//!
//! Payload (137 bytes), bech32m with the human-readable part `sfbackup` and no
//! 90-character limit:
//!
//! | Bytes | Field |
//! |---|---|
//! | 1 | version, `0x01` |
//! | 1 | FROST identifier (trustee slot, 1 to n) |
//! | 1 | threshold `t` |
//! | 1 | group size `n` |
//! | 32 | signing share (the one that matches `ak`, after any even-y negation) |
//! | 96 | the escrow's Orchard full viewing key `ak ‖ nk ‖ rivk` |
//! | 1 | network: 0 mainnet, 1 testnet, 2 regtest |
//! | 4 | birthday height, little-endian (0: unknown) |
//!
//! The string is secret: with a second trustee's string it spends the escrow,
//! and on its own it views every note.

use std::collections::BTreeMap;

use bech32::{primitives::decode::CheckedHrpstring, Bech32m, Hrp};
use orchard::keys::FullViewingKey;
use zcash_protocol::consensus::NetworkType;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    dkg,
    frost::{
        keys::{KeyPackage, PublicKeyPackage, SigningShare, VerifyingShare},
        VerifyingKey,
    },
    EscrowError,
};

/// Human-readable part of a sealForge trustee backup string.
pub const HRP: &str = "sfbackup";
const VERSION: u8 = 1;
/// Payload size: version, identifier, t, n, share, FVK, network, birthday.
pub const PAYLOAD_LEN: usize = 1 + 1 + 1 + 1 + 32 + 96 + 1 + 4;

/// One trustee's backup. The signing share is wiped when this is dropped.
pub struct Backup {
    /// FROST identifier (the trustee's slot).
    pub id: u8,
    pub t: u8,
    pub n: u8,
    share: [u8; 32],
    /// The escrow's Orchard full viewing key bytes, `ak ‖ nk ‖ rivk`.
    pub fvk: [u8; 96],
    pub network: NetworkType,
    /// Height the escrow was created at, to scan from (0 if unknown).
    pub birthday_height: u32,
}

impl Drop for Backup {
    fn drop(&mut self) {
        self.share.zeroize();
    }
}

fn bad(msg: impl Into<String>) -> EscrowError {
    EscrowError::Backup(msg.into())
}

fn network_byte(network: NetworkType) -> u8 {
    match network {
        NetworkType::Main => 0,
        NetworkType::Test => 1,
        NetworkType::Regtest => 2,
    }
}

impl Backup {
    /// Back up a trustee's key package together with the escrow's viewing key.
    /// Refuses a viewing key whose `ak` isn't the key package's group key.
    pub fn new(
        key_package: &KeyPackage,
        n: u16,
        fvk: &FullViewingKey,
        network: NetworkType,
        birthday_height: u32,
    ) -> Result<Backup, EscrowError> {
        let fvk = fvk.to_bytes();
        let ak: [u8; 32] = key_package
            .verifying_key()
            .serialize()?
            .try_into()
            .map_err(|_| bad("unexpected group key length"))?;
        if fvk[..32] != ak {
            return Err(bad(
                "the viewing key's ak is not this key package's group key",
            ));
        }
        let id = (1..=255u16)
            .find(|slot| dkg::identifier(*slot).ok() == Some(*key_package.identifier()))
            .ok_or_else(|| bad("this trustee's identifier is not a slot from 1 to 255"))?;
        let t = u8::try_from(*key_package.min_signers()).map_err(|_| bad("threshold too large"))?;
        let n = u8::try_from(n).map_err(|_| bad("group too large"))?;
        let share = Zeroizing::new(key_package.signing_share().serialize());
        let backup = Backup {
            id: id as u8,
            t,
            n,
            share: share
                .as_slice()
                .try_into()
                .map_err(|_| bad("unexpected signing share length"))?,
            fvk,
            network,
            birthday_height,
        };
        backup.check_shape()?;
        Ok(backup)
    }

    fn check_shape(&self) -> Result<(), EscrowError> {
        if self.n == 0 || self.t < 2 || self.t > self.n {
            return Err(bad(format!(
                "threshold {} of {} is not a valid group",
                self.t, self.n
            )));
        }
        if self.id == 0 || self.id > self.n {
            return Err(bad(format!(
                "identifier {} is outside 1 to {}",
                self.id, self.n
            )));
        }
        Ok(())
    }

    /// The raw payload (secret).
    pub fn payload(&self) -> Zeroizing<[u8; PAYLOAD_LEN]> {
        let mut p = Zeroizing::new([0u8; PAYLOAD_LEN]);
        p[..4].copy_from_slice(&[VERSION, self.id, self.t, self.n]);
        p[4..36].copy_from_slice(&self.share);
        p[36..132].copy_from_slice(&self.fvk);
        p[132] = network_byte(self.network);
        p[133..].copy_from_slice(&self.birthday_height.to_le_bytes());
        p
    }

    /// Parse a payload. Rejects an unknown version, a wrong length, a bad
    /// group shape, an unknown network, or a viewing key that isn't valid.
    pub fn from_payload(payload: &[u8]) -> Result<Backup, EscrowError> {
        // The version first, so a later version is reported as such.
        match payload.first() {
            Some(&VERSION) => {}
            other => return Err(bad(format!("backup version {other:?} is not supported"))),
        }
        if payload.len() != PAYLOAD_LEN {
            return Err(bad(format!(
                "a version 1 backup is {PAYLOAD_LEN} bytes, not {}",
                payload.len()
            )));
        }
        let network = match payload[132] {
            0 => NetworkType::Main,
            1 => NetworkType::Test,
            2 => NetworkType::Regtest,
            other => return Err(bad(format!("unknown network byte {other}"))),
        };
        let fvk: [u8; 96] = payload[36..132].try_into().expect("length checked");
        FullViewingKey::from_bytes(&fvk).ok_or_else(|| bad("the viewing key is not valid"))?;
        let backup = Backup {
            id: payload[1],
            t: payload[2],
            n: payload[3],
            share: payload[4..36].try_into().expect("length checked"),
            fvk,
            network,
            birthday_height: u32::from_le_bytes(payload[133..].try_into().expect("length checked")),
        };
        backup.check_shape()?;
        Ok(backup)
    }

    /// The backup string (lowercase bech32m).
    pub fn encode(&self) -> Result<String, EscrowError> {
        let hrp = Hrp::parse(HRP).expect("valid hrp");
        bech32::encode::<Bech32m>(hrp, self.payload().as_slice())
            .map_err(|e| bad(format!("encoding: {e}")))
    }

    /// Parse a backup string: bech32m only, the `sfbackup` prefix, all one
    /// case, and at most 4 zero bits of padding (ZIP 173), so each payload has
    /// exactly one spelling.
    pub fn decode(text: &str) -> Result<Backup, EscrowError> {
        let text = text.trim();
        let checked = CheckedHrpstring::new::<Bech32m>(text)
            .map_err(|e| bad(format!("not a valid backup string: {e}")))?;
        if !checked.hrp().as_str().eq_ignore_ascii_case(HRP) {
            return Err(bad(format!(
                "this is a {} string, not a sealForge trustee backup ({HRP})",
                checked.hrp()
            )));
        }
        checked
            .validate_segwit_padding()
            .map_err(|e| bad(format!("bad padding: {e}")))?;
        let payload = Zeroizing::new(checked.byte_iter().collect::<Vec<u8>>());
        Backup::from_payload(&payload)
    }

    /// The escrow's full viewing key.
    pub fn full_viewing_key(&self) -> FullViewingKey {
        FullViewingKey::from_bytes(&self.fvk).expect("checked when built")
    }

    /// The escrow's group key `ak`.
    pub fn ak(&self) -> [u8; 32] {
        self.fvk[..32].try_into().expect("96 bytes")
    }

    /// Rebuild this trustee's FROST key package. Its signing share is checked
    /// only when it signs with another trustee's (see [`recover_group`]).
    pub fn key_package(&self) -> Result<KeyPackage, EscrowError> {
        let share = SigningShare::deserialize(&self.share)?;
        let verifying_share = VerifyingShare::from(share);
        let verifying_key = VerifyingKey::deserialize(&self.ak())?;
        Ok(KeyPackage::new(
            dkg::identifier(self.id as u16)?,
            share,
            verifying_share,
            verifying_key,
            self.t as u16,
        ))
    }
}

/// Rebuild what a signing session needs from `t` trustees' backups, with no
/// coordinator: each key package, and a public key package holding these
/// trustees' verifying shares. Refuses backups from different escrows, for
/// different networks, with different thresholds, or from the same trustee.
/// Signing with the result aggregates only if every share is right for `ak`.
pub fn recover_group(
    backups: &[Backup],
) -> Result<
    (
        BTreeMap<crate::frost::Identifier, KeyPackage>,
        PublicKeyPackage,
    ),
    EscrowError,
> {
    let first = backups.first().ok_or_else(|| bad("no backups"))?;
    if backups.len() < first.t as usize {
        return Err(bad(format!(
            "{} backup(s) given; the escrow needs {} to sign",
            backups.len(),
            first.t
        )));
    }
    let mut keys = BTreeMap::new();
    let mut shares = BTreeMap::new();
    for b in backups {
        if b.fvk != first.fvk || b.network != first.network {
            return Err(bad("these backups are from different escrows"));
        }
        if (b.t, b.n) != (first.t, first.n) {
            return Err(bad("these backups disagree on the threshold"));
        }
        let kp = b.key_package()?;
        let id = *kp.identifier();
        if keys.contains_key(&id) {
            return Err(bad(format!("two backups are from trustee {}", b.id)));
        }
        shares.insert(id, *kp.verifying_share());
        keys.insert(id, kp);
    }
    let pubkeys = PublicKeyPackage::new(
        shares,
        VerifyingKey::deserialize(&first.ak())?,
        Some(first.t as u16),
    );
    Ok((keys, pubkeys))
}

/// Drill: have these backups sign a random spend authorization together, with
/// no coordinator, and check it verifies under the escrow's `ak` like an
/// on-chain spend. Proves the strings can move the funds; signs no
/// transaction.
pub fn drill_sign<R: rand::RngCore + rand::CryptoRng>(
    backups: &[Backup],
    rng: &mut R,
) -> Result<(), EscrowError> {
    use pasta_curves::group::ff::{FromUniformBytes, PrimeField};

    use crate::signing::{self, SpendAuthRequest};

    let (keys, pubkeys) = recover_group(backups)?;
    let mut wide = [0u8; 64];
    rng.fill_bytes(&mut wide);
    let mut sighash = [0u8; 32];
    rng.fill_bytes(&mut sighash);
    let request = SpendAuthRequest {
        sighash,
        alpha: pasta_curves::Fq::from_uniform_bytes(&wide).to_repr(),
    };
    let mut nonces = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for (id, kp) in &keys {
        let (n, c) = signing::commit(kp, rng);
        nonces.insert(*id, n);
        commitments.insert(*id, c);
    }
    let package = signing::signing_package(&request, commitments);
    let mut shares = BTreeMap::new();
    for (id, n) in &nonces {
        shares.insert(*id, signing::sign(&request, &package, n, &keys[id])?);
    }
    let signature = signing::aggregate(&request, &package, &shares, &pubkeys)?;
    signing::verify_spend_auth(&backups[0].ak(), &request, &signature)
}

#[cfg(test)]
mod tests {
    use rand::rngs::OsRng;
    use zcash_protocol::consensus::{NetworkType, TEST_NETWORK};

    use super::*;
    use crate::{
        address,
        signing::{self, SpendAuthRequest},
        Threshold,
    };

    fn escrow() -> (
        BTreeMap<crate::frost::Identifier, KeyPackage>,
        PublicKeyPackage,
        FullViewingKey,
    ) {
        let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let ak = dkg::group_key_bytes(&pubkeys).unwrap();
        let fvk = address::escrow_fvk(&ak, &mut OsRng).unwrap();
        (keys, pubkeys, fvk)
    }

    fn backup_of(
        slot: u16,
        keys: &BTreeMap<crate::frost::Identifier, KeyPackage>,
        fvk: &FullViewingKey,
    ) -> Backup {
        Backup::new(
            &keys[&dkg::identifier(slot).unwrap()],
            3,
            fvk,
            NetworkType::Test,
            3_900_000,
        )
        .unwrap()
    }

    fn sign(
        req: &SpendAuthRequest,
        keys: &BTreeMap<crate::frost::Identifier, KeyPackage>,
        pubkeys: &PublicKeyPackage,
    ) -> Result<[u8; 64], EscrowError> {
        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for (id, kp) in keys {
            let (n, c) = signing::commit(kp, &mut OsRng);
            nonces.insert(*id, n);
            commitments.insert(*id, c);
        }
        let package = signing::signing_package(req, commitments);
        let mut shares = BTreeMap::new();
        for (id, n) in &nonces {
            shares.insert(*id, signing::sign(req, &package, n, &keys[id])?);
        }
        signing::aggregate(req, &package, &shares, pubkeys)
    }

    fn request() -> SpendAuthRequest {
        use pasta_curves::group::ff::{FromUniformBytes, PrimeField};
        use rand::RngCore;
        let mut wide = [0u8; 64];
        OsRng.fill_bytes(&mut wide);
        let mut sighash = [0u8; 32];
        OsRng.fill_bytes(&mut sighash);
        SpendAuthRequest {
            sighash,
            alpha: pasta_curves::Fq::from_uniform_bytes(&wide).to_repr(),
        }
    }

    #[test]
    fn round_trips_and_keeps_the_escrow_address() {
        let (keys, _, fvk) = escrow();
        let b = backup_of(2, &keys, &fvk);
        let text = b.encode().unwrap();
        assert!(text.starts_with("sfbackup1"));
        let back = Backup::decode(&text).unwrap();
        assert_eq!(*back.payload(), *b.payload());
        // Upper case is the same string.
        assert_eq!(
            *Backup::decode(&text.to_uppercase()).unwrap().payload(),
            *b.payload()
        );
        assert_eq!(
            address::escrow_address(&back.full_viewing_key(), &TEST_NETWORK).unwrap(),
            address::escrow_address(&fvk, &TEST_NETWORK).unwrap()
        );
        assert_eq!(
            back.key_package().unwrap(),
            keys[&dkg::identifier(2).unwrap()]
        );
        assert_eq!(
            (back.id, back.t, back.n, back.birthday_height),
            (2, 2, 3, 3_900_000)
        );
    }

    #[test]
    fn two_backups_sign_a_spend_without_the_coordinator() {
        let (keys, _, fvk) = escrow();
        let ak = fvk.to_bytes()[..32].try_into().unwrap();
        for pair in [[1, 2], [1, 3], [2, 3]] {
            // Only the strings survive: no coordinator database, no keystore.
            let strings: Vec<String> = pair
                .iter()
                .map(|s| backup_of(*s, &keys, &fvk).encode().unwrap())
                .collect();
            let backups: Vec<Backup> = strings.iter().map(|s| Backup::decode(s).unwrap()).collect();
            let (restored, pubkeys) = recover_group(&backups).unwrap();
            assert_eq!(
                address::escrow_address(&backups[0].full_viewing_key(), &TEST_NETWORK).unwrap(),
                address::escrow_address(&fvk, &TEST_NETWORK).unwrap()
            );
            let req = request();
            let sig = sign(&req, &restored, &pubkeys).unwrap();
            signing::verify_spend_auth(&ak, &req, &sig).unwrap();
            drill_sign(&backups, &mut OsRng).unwrap();
        }
    }

    #[test]
    fn a_wrong_share_does_not_sign() {
        let (keys, _, fvk) = escrow();
        let mut tampered = backup_of(1, &keys, &fvk);
        tampered.share = backup_of(3, &keys, &fvk).share;
        let backups = vec![tampered, backup_of(2, &keys, &fvk)];
        let (restored, pubkeys) = recover_group(&backups).unwrap();
        assert!(sign(&request(), &restored, &pubkeys).is_err());
        assert!(drill_sign(&backups, &mut OsRng).is_err());
    }

    #[test]
    fn refuses_mismatched_backups() {
        let (keys, _, fvk) = escrow();
        let (other_keys, _, other_fvk) = escrow();
        let one = || backup_of(1, &keys, &fvk);
        assert!(recover_group(&[one()]).is_err(), "one backup can't sign");
        assert!(
            recover_group(&[one(), one()]).is_err(),
            "same trustee twice"
        );
        assert!(
            recover_group(&[one(), backup_of(2, &other_keys, &other_fvk)]).is_err(),
            "another escrow"
        );
        // A viewing key from another group is refused when backing up.
        assert!(Backup::new(
            &keys[&dkg::identifier(1).unwrap()],
            3,
            &other_fvk,
            NetworkType::Test,
            0
        )
        .is_err());
    }

    #[test]
    fn rejects_malformed_strings() {
        let (keys, _, fvk) = escrow();
        let b = backup_of(1, &keys, &fvk);
        let good = b.encode().unwrap();
        let hrp = Hrp::parse(HRP).unwrap();
        let with = |edit: &dyn Fn(&mut Vec<u8>)| {
            let mut p = b.payload().to_vec();
            edit(&mut p);
            bech32::encode::<Bech32m>(hrp, &p).unwrap()
        };
        let cases = [
            // A typo breaks the checksum.
            good.replacen('q', "p", 1),
            // Mixed case.
            format!("{}{}", &good[..12], good[12..].to_uppercase()),
            // Bech32 (not m) checksum.
            bech32::encode::<bech32::Bech32>(hrp, b.payload().as_slice()).unwrap(),
            // Another prefix (e.g. a zkfrost string).
            bech32::encode::<Bech32m>(Hrp::parse("zkfrost").unwrap(), b.payload().as_slice())
                .unwrap(),
            with(&|p| p[0] = 2),
            with(&|p| p.truncate(PAYLOAD_LEN - 1)),
            with(&|p| p.push(0)),
            with(&|p| p[1] = 0),
            with(&|p| p[1] = 4),
            with(&|p| p[2] = 1),
            with(&|p| p[2] = 4),
            with(&|p| p[132] = 3),
            with(&|p| p[36..68].copy_from_slice(&[0xff; 32])),
        ];
        for (i, s) in cases.iter().enumerate() {
            assert!(
                Backup::decode(s).is_err(),
                "case {i} should be rejected: {s}"
            );
        }
    }

    #[test]
    fn rejects_nonzero_padding() {
        // 137 bytes are 1096 bits: 220 five-bit groups carry 4 spare bits,
        // which must be zero. Set one and re-checksum: a second spelling.
        use bech32::{
            primitives::iter::{ByteIterExt, Fe32IterExt},
            Fe32,
        };
        let (keys, _, fvk) = escrow();
        let b = backup_of(1, &keys, &fvk);
        let mut fes: Vec<Fe32> = b.payload().iter().copied().bytes_to_fes().collect();
        let last = fes.len() - 1;
        fes[last] = Fe32::try_from(fes[last].to_u8() | 1).unwrap();
        let hrp = Hrp::parse(HRP).unwrap();
        let s: String = fes
            .into_iter()
            .with_checksum::<Bech32m>(&hrp)
            .chars()
            .collect();
        assert!(Backup::decode(&s).is_err());
    }
}
