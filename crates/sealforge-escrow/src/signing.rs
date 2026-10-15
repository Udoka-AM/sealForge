//! Rerandomized FROST signing of Orchard/Ironwood spend authorizations.
//!
//! For each escrow spend in a PCZT, the signer role exposes the shielded
//! sighash and the action's randomizer α. Trustees sign with their key package
//! randomized by α, the shares aggregate into a RedPallas `SpendAuth`
//! signature valid under `rk = ak + [α]G`, and the coordinator applies it with
//! `Signer::apply_ironwood_signature`.

use std::collections::BTreeMap;

use frost_rerandomized::{RandomizedParams, Randomizer};
use pasta_curves::group::ff::PrimeField;
use rand::{CryptoRng, RngCore};

use crate::{
    frost::{
        self,
        keys::{KeyPackage, PublicKeyPackage},
        round1::{SigningCommitments, SigningNonces},
        round2::SignatureShare,
        Identifier, SigningPackage,
    },
    EscrowError,
};

/// What trustees sign for one escrow spend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendAuthRequest {
    /// Shielded sighash from `pczt::roles::signer::Signer::shielded_sighash`.
    pub sighash: [u8; 32],
    /// The action's spend authorization randomizer α (little-endian Pallas scalar).
    pub alpha: [u8; 32],
}

/// Round 1 (each trustee): commit to fresh nonces. Nonces stay secret and are
/// used exactly once.
pub fn commit<R: RngCore + CryptoRng>(
    key_package: &KeyPackage,
    rng: &mut R,
) -> (SigningNonces, SigningCommitments) {
    frost::round1::commit(key_package.signing_share(), rng)
}

/// Coordinator: the signing package for a request, from ≥ 2 trustees' commitments.
pub fn signing_package(
    request: &SpendAuthRequest,
    commitments: BTreeMap<Identifier, SigningCommitments>,
) -> SigningPackage {
    SigningPackage::new(commitments, &request.sighash)
}

fn randomized_params(
    verifying_key: &frost::VerifyingKey,
    alpha: &[u8; 32],
) -> Result<RandomizedParams<frost::PallasBlake2b512>, EscrowError> {
    let randomizer = Randomizer::deserialize(alpha).map_err(|_| EscrowError::InvalidRandomizer)?;
    Ok(RandomizedParams::from_randomizer(verifying_key, randomizer))
}

/// Round 2 (each trustee): sign with the key package randomized by α.
/// Trustees must check the PCZT (outputs, amounts, fee) before calling this.
pub fn sign(
    request: &SpendAuthRequest,
    signing_package: &SigningPackage,
    nonces: &SigningNonces,
    key_package: &KeyPackage,
) -> Result<SignatureShare, EscrowError> {
    let randomizer =
        Randomizer::deserialize(&request.alpha).map_err(|_| EscrowError::InvalidRandomizer)?;
    // Deprecated upstream in favour of seed-derived randomizers, but Orchard
    // fixes α in the PCZT, so we must sign with exactly that randomizer.
    #[allow(deprecated)]
    let share = frost_rerandomized::sign(signing_package, nonces, key_package, randomizer)?;
    Ok(share)
}

/// One trustee's round 1 for a whole batch: serialized nonces (secret, keep
/// until round 2, then delete) and commitments (public), one per spend.
pub struct BatchCommit {
    pub nonces: Vec<Vec<u8>>,
    pub commitments: Vec<Vec<u8>>,
}

/// Round 1 for a batch of `spends` escrow spends.
pub fn commit_batch<R: RngCore + CryptoRng>(
    key_package: &KeyPackage,
    spends: usize,
    rng: &mut R,
) -> Result<BatchCommit, EscrowError> {
    let mut out = BatchCommit {
        nonces: Vec::with_capacity(spends),
        commitments: Vec::with_capacity(spends),
    };
    for _ in 0..spends {
        let (n, c) = commit(key_package, rng);
        out.nonces.push(n.serialize()?);
        out.commitments.push(c.serialize()?);
    }
    Ok(out)
}

/// Round 2 for a batch: one serialized signature share per spend, in order.
/// `signer_commitments` holds both chosen signers' round 1 commitments by slot,
/// and `nonces` this trustee's round 1 nonces.
pub fn sign_batch(
    key_package: &KeyPackage,
    sighash: [u8; 32],
    alphas: &[[u8; 32]],
    signer_commitments: &BTreeMap<u16, Vec<Vec<u8>>>,
    nonces: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, EscrowError> {
    if nonces.len() != alphas.len() {
        return Err(EscrowError::BatchShape);
    }
    let mut shares = Vec::with_capacity(alphas.len());
    for (i, alpha) in alphas.iter().enumerate() {
        let request = SpendAuthRequest {
            sighash,
            alpha: *alpha,
        };
        let mut commitments = BTreeMap::new();
        for (slot, values) in signer_commitments {
            let c = values.get(i).ok_or(EscrowError::BatchShape)?;
            commitments.insert(
                crate::dkg::identifier(*slot)?,
                SigningCommitments::deserialize(c)?,
            );
        }
        let package = signing_package(&request, commitments);
        let nonces = SigningNonces::deserialize(&nonces[i])?;
        shares.push(sign(&request, &package, &nonces, key_package)?.serialize());
    }
    Ok(shares)
}

/// Coordinator: aggregate shares into a 64-byte RedPallas `SpendAuth` signature.
pub fn aggregate(
    request: &SpendAuthRequest,
    signing_package: &SigningPackage,
    shares: &BTreeMap<Identifier, SignatureShare>,
    pubkeys: &PublicKeyPackage,
) -> Result<[u8; 64], EscrowError> {
    let params = randomized_params(pubkeys.verifying_key(), &request.alpha)?;
    let signature = frost::rerandomized::aggregate(signing_package, shares, pubkeys, &params)?;
    signature
        .serialize()?
        .try_into()
        .map_err(|_| EscrowError::BadSignature)
}

/// Verify a spend authorization signature exactly as consensus does: under
/// `rk = ak.randomize(α)` with Orchard's RedPallas `SpendAuth` (reddsa 0.6,
/// the version `orchard` uses).
pub fn verify_spend_auth(
    ak: &[u8; 32],
    request: &SpendAuthRequest,
    signature: &[u8; 64],
) -> Result<(), EscrowError> {
    let alpha = Option::<pasta_curves::Fq>::from(pasta_curves::Fq::from_repr(request.alpha))
        .ok_or(EscrowError::InvalidRandomizer)?;
    let ak = reddsa::VerificationKey::<reddsa::orchard::SpendAuth>::try_from(*ak)
        .map_err(|_| EscrowError::BadSignature)?;
    ak.randomize(&alpha)
        .verify(&request.sighash, &reddsa::Signature::from(*signature))
        .map_err(|_| EscrowError::BadSignature)
}

#[cfg(all(test, feature = "address"))]
mod tests {
    use rand::{rngs::OsRng, RngCore};
    use zcash_protocol::consensus::TEST_NETWORK;

    use super::*;
    use crate::{address, dkg, Threshold};

    fn random_alpha() -> [u8; 32] {
        // A uniformly random Pallas scalar, as the PCZT IO finalizer would set.
        use pasta_curves::group::ff::FromUniformBytes;
        let mut wide = [0u8; 64];
        OsRng.fill_bytes(&mut wide);
        pasta_curves::Fq::from_uniform_bytes(&wide).to_repr()
    }

    fn request() -> SpendAuthRequest {
        let mut sighash = [0u8; 32];
        OsRng.fill_bytes(&mut sighash);
        SpendAuthRequest {
            sighash,
            alpha: random_alpha(),
        }
    }

    fn sign_with(
        signers: &[u16],
        req: &SpendAuthRequest,
        keys: &BTreeMap<Identifier, KeyPackage>,
        pubkeys: &PublicKeyPackage,
    ) -> Result<[u8; 64], EscrowError> {
        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for &slot in signers {
            let id = dkg::identifier(slot)?;
            let (n, c) = commit(&keys[&id], &mut OsRng);
            nonces.insert(id, n);
            commitments.insert(id, c);
        }
        let package = signing_package(req, commitments);
        let mut shares = BTreeMap::new();
        for (id, n) in &nonces {
            shares.insert(*id, sign(req, &package, n, &keys[id])?);
        }
        aggregate(req, &package, &shares, pubkeys)
    }

    #[test]
    fn two_of_three_produces_consensus_valid_spend_auth_signatures() {
        let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let ak = dkg::group_key_bytes(&pubkeys).unwrap();
        assert_eq!(ak[31] >> 7, 0, "Orchard requires ak to have even Y");

        // Every 2-of-3 subset can sign, and the result verifies like an on-chain spend.
        for signers in [[1, 2], [1, 3], [2, 3]] {
            let req = request();
            let sig = sign_with(&signers, &req, &keys, &pubkeys).unwrap();
            verify_spend_auth(&ak, &req, &sig).unwrap();

            // Bound to α and to the sighash.
            let mut other_alpha = req.clone();
            other_alpha.alpha = random_alpha();
            assert!(verify_spend_auth(&ak, &other_alpha, &sig).is_err());
            let mut other_msg = req.clone();
            other_msg.sighash[0] ^= 1;
            assert!(verify_spend_auth(&ak, &other_msg, &sig).is_err());
        }
    }

    #[test]
    fn one_trustee_cannot_sign_alone() {
        let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let req = request();
        assert!(sign_with(&[1], &req, &keys, &pubkeys).is_err());
    }

    #[test]
    fn group_key_derives_escrow_address_and_view_keys() {
        let (_, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let ak = dkg::group_key_bytes(&pubkeys).unwrap();
        let fvk = address::escrow_fvk(&ak, &mut OsRng).unwrap();
        assert_eq!(
            &fvk.to_bytes()[..32],
            &ak,
            "the FVK's ak is the FROST group key"
        );

        let ua = address::escrow_address(&fvk, &TEST_NETWORK).unwrap();
        let (ufvk, uivk) = address::escrow_view_keys(&fvk, &TEST_NETWORK).unwrap();
        assert!(ua.starts_with("utest1"), "{ua}");
        assert!(ufvk.starts_with("uviewtest1"), "{ufvk}");
        assert!(uivk.starts_with("uivktest1"), "{uivk}");
    }
}
