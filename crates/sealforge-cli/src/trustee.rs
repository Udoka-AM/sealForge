//! Trustee commands (task 2.4): join with an invite, then run the DKG.
//!
//! `dkg_step` moves the local DKG forward by at most one round and is safe to
//! re-run: every secret and outgoing package is saved before it is sent, so a
//! failed request is simply retried. Packages relayed by the coordinator are
//! checked against the sending trustee's signature, and round 2 packages are
//! sealed so only the recipient can read them.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, ensure, Context};
use rand::rngs::OsRng;
use sealforge_api::{
    auth::{sign_dkg_package, verify_dkg_package},
    types::{
        AuctionStatus, CompleteDkg, DkgPackage, DkgState, JoinAsTrustee, PostRound1, PostRound2,
        Round2Out, TrusteeJoined,
    },
};
use sealforge_escrow::{
    dkg,
    frost::{
        keys::{dkg as frost_dkg, KeyPackage, PublicKeyPackage},
        Identifier,
    },
    Threshold,
};
use sha2::{Digest, Sha256};

use crate::{
    client::Client,
    keystore::{Keystore, TrusteeIdentity},
};

const ROUND1_SECRET: &str = "round1.secret";
const ROUND1_PACKAGE: &str = "round1.package";
const ROUND2_SECRET: &str = "round2.secret";
const ROUND2_OUT: &str = "round2.out.json";
const KEY_PACKAGE: &str = "key-package.hex";
const PUBLIC_KEY_PACKAGE: &str = "public-key-package.hex";

/// Where this trustee's DKG stands after a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DkgProgress {
    WaitingForTrustees {
        joined: usize,
        total: usize,
    },
    SentRound1,
    WaitingForRound1 {
        received: usize,
        total: usize,
    },
    SentRound2,
    WaitingForRound2 {
        received: usize,
        needed: usize,
    },
    /// This trustee finished. `fingerprint` must match across trustees (compare out of band).
    Completed {
        fingerprint: String,
        escrow_ua: Option<String>,
    },
    Ready {
        escrow_ua: String,
    },
}

/// `sealforge trustee join <invite>`
pub async fn join(
    store: &Keystore,
    coordinator: &str,
    invite_code: &str,
) -> anyhow::Result<TrusteeJoined> {
    let signing = ed25519_dalek::SigningKey::generate(&mut OsRng);
    let enc = crypto_box::SecretKey::generate(&mut crypto_box::aead::OsRng);
    let client = Client::new(coordinator, Some(signing.clone()));
    let joined: TrusteeJoined = client
        .post(
            "/v1/trustees/join",
            &JoinAsTrustee {
                invite_code: invite_code.to_owned(),
                enc_pubkey: hex::encode(enc.public_key().as_bytes()),
            },
        )
        .await?;
    store
        .save_trustee_identity(&TrusteeIdentity {
            auction_code: joined.auction_code.clone(),
            slot: joined.slot,
            label: joined.label.clone(),
            signing_key: hex::encode(signing.to_bytes()),
            enc_secret: hex::encode(enc.to_bytes()),
        })
        .context(
            "joined, but could not save the trustee identity; ask the organiser for a new invite",
        )?;
    Ok(joined)
}

/// Short fingerprint of the group's public key package, for out-of-band comparison.
pub fn fingerprint(public_key_package: &[u8]) -> String {
    let digest = Sha256::digest(public_key_package);
    digest[..16]
        .chunks(2)
        .map(hex::encode)
        .collect::<Vec<_>>()
        .join("-")
}

fn trustee_pubkey(state: &DkgState, slot: u16) -> anyhow::Result<[u8; 32]> {
    let hex_key = state
        .trustees
        .iter()
        .find(|t| t.slot == slot)
        .and_then(|t| t.pubkey.clone())
        .ok_or_else(|| anyhow!("trustee {slot} has no key"))?;
    hex::decode(hex_key)?
        .try_into()
        .map_err(|_| anyhow!("trustee {slot} key is malformed"))
}

fn checked_payload(state: &DkgState, package: &DkgPackage, round: u8) -> anyhow::Result<Vec<u8>> {
    let payload = hex::decode(&package.payload)?;
    let signature: [u8; 64] = hex::decode(&package.signature)?
        .try_into()
        .map_err(|_| anyhow!("malformed signature"))?;
    let sender = trustee_pubkey(state, package.from_slot)?;
    ensure!(
        verify_dkg_package(&sender, &signature, &state.auction_code, round, package.from_slot, package.to_slot, &payload),
        "round {round} package from trustee {} has a bad signature; stop and tell the other trustees",
        package.from_slot
    );
    Ok(payload)
}

fn others_round1(
    state: &DkgState,
) -> anyhow::Result<BTreeMap<Identifier, frost_dkg::round1::Package>> {
    let mut map = BTreeMap::new();
    for package in state.round1.iter().filter(|p| p.from_slot != state.my_slot) {
        let payload = checked_payload(state, package, 1)?;
        map.insert(
            dkg::identifier(package.from_slot)?,
            frost_dkg::round1::Package::deserialize(&payload)?,
        );
    }
    Ok(map)
}

/// Advance this trustee's DKG by at most one round.
pub async fn dkg_step(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<DkgProgress> {
    let identity = store.trustee_identity(code)?;
    let signing = identity.signing_key()?;
    let client = Client::new(coordinator, Some(signing.clone()));
    let path = format!("/v1/auctions/{code}/dkg");
    let state: DkgState = client.get(&path).await?;
    let me = state.my_slot;
    let threshold = Threshold {
        min_signers: state.min_signers,
        max_signers: state.max_signers,
    };
    let total = state.max_signers as usize;

    match state.auction_status {
        AuctionStatus::Draft => {
            let joined = state.trustees.iter().filter(|t| t.pubkey.is_some()).count();
            return Ok(DkgProgress::WaitingForTrustees { joined, total });
        }
        AuctionStatus::Dkg => {}
        _ => {
            ensure!(
                store.read(code, KEY_PACKAGE)?.is_some(),
                "key generation finished without this machine's key share"
            );
            return Ok(DkgProgress::Ready {
                escrow_ua: state.escrow_ua.clone().unwrap_or_default(),
            });
        }
    }

    // Round 1: create (once), then (re)send until the coordinator has it.
    if store.read(code, ROUND1_SECRET)?.is_none() {
        let (secret, package) = dkg::round1(me, threshold, &mut OsRng)?;
        store.write(code, ROUND1_SECRET, &secret.serialize()?)?;
        store.write(code, ROUND1_PACKAGE, &package.serialize()?)?;
    }
    if !state.round1.iter().any(|p| p.from_slot == me) {
        let payload = store
            .read(code, ROUND1_PACKAGE)?
            .ok_or_else(|| anyhow!("missing round 1 package"))?;
        let signature = sign_dkg_package(&signing, code, 1, me, None, &payload);
        let _: DkgState = client
            .post(
                &format!("{path}/round1"),
                &PostRound1 {
                    payload: hex::encode(&payload),
                    signature: hex::encode(signature),
                },
            )
            .await?;
        return Ok(DkgProgress::SentRound1);
    }
    if state.round1.len() < total {
        return Ok(DkgProgress::WaitingForRound1 {
            received: state.round1.len(),
            total,
        });
    }
    let round1 = others_round1(&state)?;

    // Round 2: one sealed, signed package per other trustee.
    if store.read(code, ROUND2_SECRET)?.is_none() {
        let secret1 = frost_dkg::round1::SecretPackage::deserialize(
            &store
                .read(code, ROUND1_SECRET)?
                .ok_or_else(|| anyhow!("missing round 1 secret"))?,
        )?;
        let (secret2, outgoing) = dkg::round2(secret1, &round1)?;
        let mut out = Vec::new();
        for trustee in state.trustees.iter().filter(|t| t.slot != me) {
            let package = outgoing
                .get(&dkg::identifier(trustee.slot)?)
                .ok_or_else(|| anyhow!("no round 2 package for trustee {}", trustee.slot))?;
            let enc_pubkey: [u8; 32] =
                hex::decode(trustee.enc_pubkey.as_deref().unwrap_or_default())?
                    .try_into()
                    .map_err(|_| anyhow!("trustee {} has no encryption key", trustee.slot))?;
            let sealed = crypto_box::PublicKey::from(enc_pubkey)
                .seal(&mut crypto_box::aead::OsRng, &package.serialize()?)
                .map_err(|_| anyhow!("sealing the round 2 package failed"))?;
            let signature = sign_dkg_package(&signing, code, 2, me, Some(trustee.slot), &sealed);
            out.push(Round2Out {
                to_slot: trustee.slot,
                payload: hex::encode(sealed),
                signature: hex::encode(signature),
            });
        }
        store.write(code, ROUND2_SECRET, &secret2.serialize()?)?;
        store.write(
            code,
            ROUND2_OUT,
            serde_json::to_string(&PostRound2 { packages: out })?.as_bytes(),
        )?;
    }
    let my_status = state
        .trustees
        .iter()
        .find(|t| t.slot == me)
        .map(|t| t.dkg_status);
    if matches!(
        my_status,
        Some(sealforge_api::types::DkgStatus::Invited | sealforge_api::types::DkgStatus::Round1)
    ) {
        let body: PostRound2 = serde_json::from_slice(
            &store
                .read(code, ROUND2_OUT)?
                .ok_or_else(|| anyhow!("missing round 2 packages"))?,
        )?;
        let _: DkgState = client.post(&format!("{path}/round2"), &body).await?;
        return Ok(DkgProgress::SentRound2);
    }

    // Round 3: open the packages sent to us and derive our key share.
    if store.read(code, KEY_PACKAGE)?.is_none() {
        let needed = total - 1;
        if state.round2_for_me.len() < needed {
            return Ok(DkgProgress::WaitingForRound2 {
                received: state.round2_for_me.len(),
                needed,
            });
        }
        let enc = identity.enc_secret()?;
        let mut round2 = BTreeMap::new();
        for package in &state.round2_for_me {
            ensure!(
                package.to_slot == Some(me),
                "received a package addressed to someone else"
            );
            let sealed = checked_payload(&state, package, 2)?;
            let opened = enc.unseal(&sealed).map_err(|_| {
                anyhow!(
                    "could not open the round 2 package from trustee {}",
                    package.from_slot
                )
            })?;
            round2.insert(
                dkg::identifier(package.from_slot)?,
                frost_dkg::round2::Package::deserialize(&opened)?,
            );
        }
        let secret2 = frost_dkg::round2::SecretPackage::deserialize(
            &store
                .read(code, ROUND2_SECRET)?
                .ok_or_else(|| anyhow!("missing round 2 secret"))?,
        )?;
        let (key_package, pubkeys) = dkg::finish(&secret2, &round1, &round2)?;
        store.write(
            code,
            KEY_PACKAGE,
            hex::encode(key_package.serialize()?).as_bytes(),
        )?;
        store.write(
            code,
            PUBLIC_KEY_PACKAGE,
            hex::encode(pubkeys.serialize()?).as_bytes(),
        )?;
    }

    let pubkeys = hex::decode(
        store
            .read(code, PUBLIC_KEY_PACKAGE)?
            .ok_or_else(|| anyhow!("missing public key package"))?,
    )?;
    let after: DkgState = client
        .post(
            &format!("{path}/complete"),
            &CompleteDkg {
                public_key_package: hex::encode(&pubkeys),
            },
        )
        .await?;
    Ok(DkgProgress::Completed {
        fingerprint: fingerprint(&pubkeys),
        escrow_ua: after.escrow_ua,
    })
}

/// Load this trustee's FROST key share and the group's public key package.
pub fn key_share(store: &Keystore, code: &str) -> anyhow::Result<(KeyPackage, PublicKeyPackage)> {
    let read = |name| -> anyhow::Result<Vec<u8>> {
        let bytes = store
            .read(code, name)?
            .ok_or_else(|| anyhow!("no {name} for {code}; finish the DKG first"))?;
        Ok(hex::decode(bytes)?)
    };
    let key_package = KeyPackage::deserialize(&read(KEY_PACKAGE)?)?;
    let pubkeys = PublicKeyPackage::deserialize(&read(PUBLIC_KEY_PACKAGE)?)?;
    if key_package.verifying_key() != pubkeys.verifying_key() {
        bail!("local key share does not match the group key");
    }
    Ok((key_package, pubkeys))
}

// ---------------------------------------------------------------------------
// Signing escrow batches (task 3.9).

use sealforge_api::types::{BatchView, PostCommitments, PostShares};
use sealforge_escrow::signing;

/// What one `sign_step` did for a batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignProgress {
    Committed { batch: String },
    Signed { batch: String },
    WaitingForSecondSigner { batch: String },
    NotSelected { batch: String },
    Done { batch: String },
}

impl std::fmt::Display for SignProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let short = |id: &str| id.chars().take(8).collect::<String>();
        match self {
            Self::Committed { batch } => write!(
                f,
                "Batch {}: commitments sent. Run again once a second trustee has committed.",
                short(batch)
            ),
            Self::Signed { batch } => write!(f, "Batch {}: signature share sent.", short(batch)),
            Self::WaitingForSecondSigner { batch } => {
                write!(f, "Batch {}: waiting for a second trustee.", short(batch))
            }
            Self::NotSelected { batch } => write!(
                f,
                "Batch {}: two other trustees are signing; you're not needed.",
                short(batch)
            ),
            Self::Done { batch } => {
                write!(f, "Batch {}: nothing left for you to do.", short(batch))
            }
        }
    }
}

fn nonces_file(batch: &str) -> String {
    format!("batch-{batch}.nonces")
}

fn hex32(value: &str) -> anyhow::Result<[u8; 32]> {
    hex::decode(value)?
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes"))
}

/// One-line summary trustees review before signing.
pub fn describe_batch(b: &BatchView) -> String {
    format!(
        "{} batch {}: {} output(s), {} ZEC total, {} escrow spend(s) to authorize",
        b.kind,
        &b.id[..8],
        b.output_count,
        sealforge_core::zip321::format_zec(sealforge_core::Zatoshi(b.total_out_zat)),
        b.spends.len()
    )
}

/// Move this trustee's part of every pending batch forward by one round.
pub async fn sign_step(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<Vec<SignProgress>> {
    let identity = store.trustee_identity(code)?;
    let (key_package, _) = key_share(store, code)?;
    let client = Client::new(coordinator, Some(identity.signing_key()?));
    let batches: Vec<BatchView> = client.get(&format!("/v1/auctions/{code}/batches")).await?;
    let me = identity.slot;
    let mut out = Vec::new();

    for batch in batches {
        if batch.status != "awaiting_signatures" {
            out.push(SignProgress::Done { batch: batch.id });
            continue;
        }
        if !batch.i_committed {
            // Round 1: fresh nonces per spend, saved (owner-only) before anything is sent.
            let round1 = signing::commit_batch(&key_package, batch.spends.len(), &mut OsRng)?;
            let nonces: Vec<String> = round1.nonces.iter().map(hex::encode).collect();
            let commitments: Vec<String> = round1.commitments.iter().map(hex::encode).collect();
            // Nonces from a round the coordinator reset are discarded, never reused.
            std::fs::remove_file(store.trustee_dir(code).join(nonces_file(&batch.id))).ok();
            store.write(
                code,
                &nonces_file(&batch.id),
                serde_json::to_string(&nonces)?.as_bytes(),
            )?;
            let _: BatchView = client
                .post(
                    &format!("/v1/batches/{}/commitments", batch.id),
                    &PostCommitments { commitments },
                )
                .await?;
            out.push(SignProgress::Committed { batch: batch.id });
            continue;
        }
        if !batch.signer_slots.contains(&me) {
            let progress = if batch.signer_slots.is_empty() {
                SignProgress::WaitingForSecondSigner { batch: batch.id }
            } else {
                SignProgress::NotSelected { batch: batch.id }
            };
            out.push(progress);
            continue;
        }
        if batch.i_shared {
            out.push(SignProgress::Done { batch: batch.id });
            continue;
        }
        if batch.signer_commitments.len() < 2 {
            out.push(SignProgress::WaitingForSecondSigner { batch: batch.id });
            continue;
        }

        // Round 2: sign each spend with its alpha against the agreed commitments.
        let nonce_hex: Vec<String> =
            serde_json::from_slice(&store.read(code, &nonces_file(&batch.id))?.ok_or_else(
                || anyhow!("nonces for batch {} are missing on this machine", batch.id),
            )?)?;
        let alphas = batch
            .spends
            .iter()
            .map(|s| hex32(&s.alpha))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let commitments = batch
            .signer_commitments
            .iter()
            .map(|(slot, values)| {
                Ok((
                    *slot,
                    values
                        .iter()
                        .map(hex::decode)
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            })
            .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
        let nonces = nonce_hex
            .iter()
            .map(hex::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let shares: Vec<String> = signing::sign_batch(
            &key_package,
            hex32(&batch.sighash)?,
            &alphas,
            &commitments,
            &nonces,
        )?
        .iter()
        .map(hex::encode)
        .collect();
        let _: BatchView = client
            .post(
                &format!("/v1/batches/{}/shares", batch.id),
                &PostShares { shares },
            )
            .await?;
        // Nonces must never be reused.
        std::fs::remove_file(store.trustee_dir(code).join(nonces_file(&batch.id))).ok();
        out.push(SignProgress::Signed { batch: batch.id });
    }
    Ok(out)
}

/// Pending batches as this trustee sees them (for review before signing).
pub async fn pending_batches(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<Vec<BatchView>> {
    let identity = store.trustee_identity(code)?;
    Client::new(coordinator, Some(identity.signing_key()?))
        .get(&format!("/v1/auctions/{code}/batches"))
        .await
}
