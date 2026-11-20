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
    auth::{sign_dkg_package, sign_escrow_attestation, verify_dkg_package},
    types::{
        AuctionStatus, CompleteDkg, DkgPackage, DkgState, JoinAsTrustee, PostRound1, PostRound2,
        Round2Out, TrusteeJoined, VerifyEscrow,
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
/// The escrow address this trustee checked and signed.
const ESCROW_ATTESTED: &str = "escrow-attested.txt";

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
    /// This step rebuilt the escrow address from this trustee's own group key,
    /// found it matches, and sent a signed attestation.
    Verified {
        escrow_ua: String,
    },
    /// Key generation is done and this trustee already verified the escrow.
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
        AuctionStatus::Cancelled => bail!("the organiser cancelled auction {code}"),
        _ => {
            ensure!(
                store.read(code, KEY_PACKAGE)?.is_some(),
                "key generation finished without this machine's key share"
            );
            let escrow_ua = state.escrow_ua.clone().unwrap_or_default();
            let verified = state
                .trustees
                .iter()
                .any(|t| t.slot == me && t.escrow_verified);
            if state.auction_status == AuctionStatus::Ready && !verified {
                verify_escrow(store, &client, &identity, code, &state).await?;
                return Ok(DkgProgress::Verified { escrow_ua });
            }
            return Ok(DkgProgress::Ready { escrow_ua });
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
    // Already reported: nothing to send again while the others finish.
    if my_status == Some(sealforge_api::types::DkgStatus::Complete) {
        return Ok(DkgProgress::Completed {
            fingerprint: fingerprint(&pubkeys),
            escrow_ua: state.escrow_ua.clone(),
        });
    }
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

/// Rebuild the escrow address from this trustee's own group key and the
/// `nk ‖ rivk` the coordinator sealed to it, and attest to it only if it
/// matches the address the coordinator shows. A coordinator showing the
/// address of a wallet it controls alone fails here. The view secret is used
/// in memory only and never written to disk.
async fn verify_escrow(
    store: &Keystore,
    client: &Client,
    identity: &TrusteeIdentity,
    code: &str,
    state: &DkgState,
) -> anyhow::Result<()> {
    use sealforge_escrow::address::{self, EncodingParams};
    use zcash_protocol::consensus::NetworkType;

    let shown = state
        .escrow_ua
        .as_deref()
        .ok_or_else(|| anyhow!("the coordinator shows no escrow address"))?;
    let sealed = hex::decode(state.escrow_view_secret_for_me.as_deref().ok_or_else(|| {
        anyhow!(
            "the coordinator sent no escrow view secret, so the escrow address can't be checked. \
             Don't let auction {code} open; tell the organiser and the other trustees."
        )
    })?)?;
    let view_secret: [u8; 64] = identity
        .enc_secret()?
        .unseal(&sealed)
        .map_err(|_| anyhow!("could not open the escrow view secret"))?
        .try_into()
        .map_err(|_| anyhow!("the escrow view secret has the wrong length"))?;
    let (_, pubkeys) = key_share(store, code)?;
    let ak = dkg::group_key_bytes(&pubkeys)?;
    let fvk = address::escrow_fvk_from_parts(&ak, &view_secret)?;
    let matches = [NetworkType::Main, NetworkType::Test, NetworkType::Regtest]
        .into_iter()
        .any(|n| address::escrow_address(&fvk, &EncodingParams(n)).is_ok_and(|ua| ua == shown));
    ensure!(
        matches,
        "ESCROW ADDRESS MISMATCH for auction {code}: the address the coordinator shows ({shown}) \
         does not come from the trustees' group key. Do not send funds to it, don't let the \
         auction open, and tell the organiser and the other trustees now."
    );
    let signature = sign_escrow_attestation(&identity.signing_key()?, code, state.my_slot, shown);
    // Remember what we signed, so later commands can check the public page
    // shows bidders this same address (`check_public`).
    store.write(code, ESCROW_ATTESTED, shown.as_bytes())?;
    let _: DkgState = client
        .post(
            &format!("/v1/auctions/{code}/dkg/verify-escrow"),
            &VerifyEscrow {
                escrow_ua: shown.to_string(),
                signature: hex::encode(signature),
            },
        )
        .await?;
    Ok(())
}

/// What [`check_public`] found on the auction's public page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicCheck {
    /// The page shows bidders the address this trustee signed, lists this
    /// trustee's own key, and every trustee's attestation verifies.
    Matches {
        escrow_ua: String,
        signed: u32,
        total: u32,
    },
    /// Bidders can't see the auction yet (it opens later).
    NotPublicYet,
    /// This trustee never attested (an auction from before attestations).
    NotAttested,
}

/// This trustee's Ed25519 key for `code` (hex), to share with bidders over a
/// channel the coordinator doesn't control.
pub fn my_key(store: &Keystore, code: &str) -> anyhow::Result<String> {
    let identity = store.trustee_identity(code)?;
    Ok(hex::encode(
        identity.signing_key()?.verifying_key().to_bytes(),
    ))
}

/// Check the auction's public page, the one bidders pay from, against what
/// this trustee verified: the same escrow address, this trustee's own key in
/// its slot, and valid attestations from every trustee. Errors on a mismatch.
///
/// This catches a coordinator that shows bidders a different address, or that
/// lists a key it controls in this trustee's place. It can't catch one that
/// serves this trustee a different page from everyone else.
pub async fn check_public(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<PublicCheck> {
    let Some(attested) = store.read(code, ESCROW_ATTESTED)? else {
        return Ok(PublicCheck::NotAttested);
    };
    let attested = String::from_utf8(attested)?;
    let identity = store.trustee_identity(code)?;
    let mine = my_key(store, code)?;
    let view: sealforge_api::types::AuctionView = match Client::new(coordinator, None)
        .get(&format!("/v1/auctions/{code}"))
        .await
    {
        Ok(view) => view,
        Err(e) if e.to_string().starts_with("404") => return Ok(PublicCheck::NotPublicYet),
        Err(e) => return Err(e),
    };
    let shown = view.escrow_ua.clone().unwrap_or_default();
    ensure!(
        shown == attested,
        "PUBLIC PAGE MISMATCH for auction {code}: bidders are shown the escrow address \
         {shown}, but you checked and signed {attested}. Bids sent there don't reach the \
         trustees. Tell the organiser, the other trustees and bidders now, and don't sign."
    );
    let check = sealforge_auditor::check_escrow_view(&view, &[(identity.slot, mine.clone())]);
    if !check.verified() {
        let problems: Vec<String> = check
            .trustees
            .iter()
            .filter(|t| t.verdict != sealforge_auditor::TrusteeVerdict::Signed)
            .map(|t| format!("slot {} ({}): {:?}", t.slot, t.label, t.verdict))
            .collect();
        bail!(
            "PUBLIC PAGE MISMATCH for auction {code}: the trustee attestations bidders see \
             don't check out ({}). Your key is {mine}. Tell the organiser and the other \
             trustees, and don't sign.",
            if problems.is_empty() {
                "no trustees listed".to_owned()
            } else {
                problems.join("; ")
            }
        );
    }
    Ok(PublicCheck::Matches {
        escrow_ua: shown,
        signed: check.signed,
        total: check.total,
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
    Committed {
        batch: String,
    },
    Signed {
        batch: String,
    },
    WaitingForSecondSigner {
        batch: String,
    },
    NotSelected {
        batch: String,
    },
    /// The coordinator now shows different round 2 inputs than the ones this
    /// trustee already signed. Signing again would reuse nonces, so it waits
    /// for the round to reset and start over with fresh ones.
    InputsChanged {
        batch: String,
    },
    Done {
        batch: String,
    },
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
            Self::InputsChanged { batch } => write!(
                f,
                "Batch {}: the coordinator changed what it is asking you to sign after you signed. \
                 Not signing again with the same nonces. The round restarts after 30 minutes; \
                 if this repeats, treat the coordinator as compromised.",
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

fn signed_file(batch: &str) -> String {
    format!("batch-{batch}.signed")
}

/// What this trustee signed in round 2, kept so a failed send can be retried
/// with the same bytes instead of signing again with the same nonces.
#[derive(serde::Serialize, serde::Deserialize)]
struct SignedRecord {
    /// [`signing_inputs_digest`] of the round 2 inputs, hex.
    inputs: String,
    shares: Vec<String>,
}

/// Digest of everything a round 2 share depends on besides the key and the
/// nonces: the sighash, each spend's alpha, and every signer's commitments.
fn signing_inputs_digest(
    sighash: &[u8; 32],
    alphas: &[[u8; 32]],
    commitments: &BTreeMap<u16, Vec<Vec<u8>>>,
) -> String {
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
    hex::encode(h.finalize())
}

async fn post_shares(client: &Client, batch: &str, shares: Vec<String>) -> anyhow::Result<()> {
    let _: BatchView = client
        .post(
            &format!("/v1/batches/{batch}/shares"),
            &PostShares { shares },
        )
        .await?;
    Ok(())
}

fn hex32(value: &str) -> anyhow::Result<[u8; 32]> {
    hex::decode(value)?
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes"))
}

/// One-line summary trustees review before signing.
pub fn describe_batch(b: &BatchView) -> String {
    if b.kind == "consolidate" {
        return format!(
            "note merge batch {}: merges {} escrow notes into one note inside the escrow \
             ({} ZEC; nothing leaves except the fee), sighash {}",
            &b.id[..8],
            b.spends.len(),
            sealforge_core::zip321::format_zec(sealforge_core::Zatoshi(b.total_out_zat)),
            &b.sighash[..b.sighash.len().min(16)]
        );
    }
    format!(
        "{} batch {}: {} output(s), {} ZEC total, {} escrow spend(s) to authorize, sighash {}",
        b.kind,
        &b.id[..8],
        b.output_count,
        sealforge_core::zip321::format_zec(sealforge_core::Zatoshi(b.total_out_zat)),
        b.spends.len(),
        &b.sighash[..b.sighash.len().min(16)]
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
    // Never co-sign money out of an escrow whose public page showed bidders
    // something other than what this trustee verified.
    check_public(store, coordinator, code).await?;
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
            // The signed record goes too: its nonces are already gone, and the
            // new round signs with new ones.
            std::fs::remove_file(store.trustee_dir(code).join(nonces_file(&batch.id))).ok();
            std::fs::remove_file(store.trustee_dir(code).join(signed_file(&batch.id))).ok();
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
        let sighash = hex32(&batch.sighash)?;
        let inputs = signing_inputs_digest(&sighash, &alphas, &commitments);

        // A share made with these nonces may already have left this machine
        // (the POST below can fail after the coordinator received it). Never
        // sign again with them: resend the exact same bytes, and only for the
        // exact same inputs. Different inputs over the same nonces would let
        // the coordinator solve for this trustee's signing share.
        if let Some(bytes) = store.read(code, &signed_file(&batch.id))? {
            let record: SignedRecord = serde_json::from_slice(&bytes)?;
            if record.inputs != inputs {
                out.push(SignProgress::InputsChanged { batch: batch.id });
                continue;
            }
            post_shares(&client, &batch.id, record.shares).await?;
            out.push(SignProgress::Signed { batch: batch.id });
            continue;
        }

        let nonce_hex: Vec<String> =
            serde_json::from_slice(&store.read(code, &nonces_file(&batch.id))?.ok_or_else(
                || anyhow!("nonces for batch {} are missing on this machine", batch.id),
            )?)?;
        let nonces = nonce_hex
            .iter()
            .map(hex::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let shares: Vec<String> =
            signing::sign_batch(&key_package, sighash, &alphas, &commitments, &nonces)?
                .iter()
                .map(hex::encode)
                .collect();
        // Consume the nonces before the shares leave this machine: record what
        // was signed (synced to disk), then delete the nonces. If either step
        // fails, nothing is sent.
        store.write(
            code,
            &signed_file(&batch.id),
            &serde_json::to_vec(&SignedRecord {
                inputs,
                shares: shares.clone(),
            })?,
        )?;
        let nonce_path = store.trustee_dir(code).join(nonces_file(&batch.id));
        std::fs::remove_file(&nonce_path)
            .with_context(|| format!("deleting used nonces {}", nonce_path.display()))?;
        post_shares(&client, &batch.id, shares).await?;
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
