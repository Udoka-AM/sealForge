//! Aggregating trustees' FROST shares for an escrow batch (task 3.9).

use std::collections::BTreeMap;

use anyhow::{anyhow, ensure};
use sealforge_api::types::SpendAuthView;
use sealforge_escrow::{
    dkg,
    frost::{keys::PublicKeyPackage, round1::SigningCommitments, round2::SignatureShare},
    signing::{self, SpendAuthRequest},
};

/// Round 1 and round 2 values one signer posted (hex, one per spend).
#[derive(Clone, Debug, Default)]
pub struct SignerValues {
    pub commitments: Vec<String>,
    pub shares: Vec<String>,
}

fn hex32(value: &str) -> anyhow::Result<[u8; 32]> {
    hex::decode(value)?
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes"))
}

/// One aggregated RedPallas `SpendAuth` signature per spend: (action index, signature).
pub fn aggregate(
    sighash: [u8; 32],
    spends: &[SpendAuthView],
    pubkeys: &PublicKeyPackage,
    signers: &BTreeMap<u16, SignerValues>,
) -> anyhow::Result<Vec<(usize, [u8; 64])>> {
    ensure!(signers.len() >= 2, "need two signers");
    let mut out = Vec::with_capacity(spends.len());
    for (i, spend) in spends.iter().enumerate() {
        let request = SpendAuthRequest {
            sighash,
            alpha: hex32(&spend.alpha)?,
        };
        let mut commitments = BTreeMap::new();
        let mut shares = BTreeMap::new();
        for (slot, values) in signers {
            let id = dkg::identifier(*slot)?;
            let c = values
                .commitments
                .get(i)
                .ok_or_else(|| anyhow!("missing commitment"))?;
            let s = values
                .shares
                .get(i)
                .ok_or_else(|| anyhow!("missing share"))?;
            commitments.insert(id, SigningCommitments::deserialize(&hex::decode(c)?)?);
            shares.insert(id, SignatureShare::deserialize(&hex::decode(s)?)?);
        }
        let package = signing::signing_package(&request, commitments);
        let signature = signing::aggregate(&request, &package, &shares, pubkeys)?;
        out.push((spend.action_index as usize, signature));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use rand::rngs::OsRng;
    use sealforge_escrow::{signing::verify_spend_auth, Threshold};

    use super::*;

    /// Trustees 1 and 3 sign two spends exactly as the CLI does; aggregation
    /// yields signatures that pass Orchard's consensus check.
    #[test]
    fn aggregates_posted_values_into_valid_signatures() {
        let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let ak = dkg::group_key_bytes(&pubkeys).unwrap();
        let sighash = [9u8; 32];
        let alphas: Vec<[u8; 32]> = (0..2)
            .map(|_| {
                use pasta_curves::group::ff::{FromUniformBytes, PrimeField};
                let mut wide = [0u8; 64];
                rand::RngCore::fill_bytes(&mut OsRng, &mut wide);
                pasta_curves::Fq::from_uniform_bytes(&wide).to_repr()
            })
            .collect();
        let spends: Vec<SpendAuthView> = alphas
            .iter()
            .enumerate()
            .map(|(i, a)| SpendAuthView {
                action_index: (i * 2) as u32,
                alpha: hex::encode(a),
            })
            .collect();

        // Round 1 (both signers), then round 2 with the agreed commitments.
        let mut nonces = BTreeMap::new();
        let mut signers: BTreeMap<u16, SignerValues> = BTreeMap::new();
        for slot in [1u16, 3] {
            let kp = &keys[&dkg::identifier(slot).unwrap()];
            for (i, _) in spends.iter().enumerate() {
                let (n, c) = signing::commit(kp, &mut OsRng);
                nonces.insert((slot, i), n);
                signers
                    .entry(slot)
                    .or_default()
                    .commitments
                    .push(hex::encode(c.serialize().unwrap()));
            }
        }
        for slot in [1u16, 3] {
            let kp = &keys[&dkg::identifier(slot).unwrap()];
            for (i, alpha) in alphas.iter().enumerate() {
                let request = SpendAuthRequest {
                    sighash,
                    alpha: *alpha,
                };
                let commitments = signers
                    .iter()
                    .map(|(s, v)| {
                        let c = SigningCommitments::deserialize(
                            &hex::decode(&v.commitments[i]).unwrap(),
                        )
                        .unwrap();
                        (dkg::identifier(*s).unwrap(), c)
                    })
                    .collect();
                let package = signing::signing_package(&request, commitments);
                let share = signing::sign(&request, &package, &nonces[&(slot, i)], kp).unwrap();
                signers
                    .get_mut(&slot)
                    .unwrap()
                    .shares
                    .push(hex::encode(share.serialize()));
            }
        }

        let signatures = aggregate(sighash, &spends, &pubkeys, &signers).unwrap();
        assert_eq!(
            signatures.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![0, 2]
        );
        for ((_, sig), alpha) in signatures.iter().zip(&alphas) {
            verify_spend_auth(
                &ak,
                &SpendAuthRequest {
                    sighash,
                    alpha: *alpha,
                },
                sig,
            )
            .unwrap();
        }
    }
}
