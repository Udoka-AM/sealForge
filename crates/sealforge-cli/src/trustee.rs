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
/// The escrow's full viewing key, kept once this trustee has checked that it
/// gives the address it attested: with its key share, what it needs to spend
/// from the escrow with another trustee if the coordinator is gone.
const ESCROW_VIEW: &str = "escrow-view.json";

/// What [`ESCROW_VIEW`] holds. Viewing power over the escrow, not spending.
#[derive(serde::Serialize, serde::Deserialize)]
struct SavedEscrowView {
    /// `ak ‖ nk ‖ rivk`, hex.
    fvk: String,
    /// `mainnet`, `testnet` or `regtest`.
    network: String,
    escrow_ua: String,
    /// Height the escrow was created at, if the coordinator said.
    #[serde(default)]
    birthday_height: Option<u32>,
}

fn network_name(n: zcash_protocol::consensus::NetworkType) -> &'static str {
    use zcash_protocol::consensus::NetworkType;
    match n {
        NetworkType::Main => "mainnet",
        NetworkType::Test => "testnet",
        NetworkType::Regtest => "regtest",
    }
}

fn network_from_name(name: &str) -> anyhow::Result<zcash_protocol::consensus::NetworkType> {
    use zcash_protocol::consensus::NetworkType;
    Ok(match name {
        "mainnet" => NetworkType::Main,
        "testnet" => NetworkType::Test,
        "regtest" => NetworkType::Regtest,
        other => bail!("unknown network {other:?}"),
    })
}

/// Keep the escrow's viewing key once it checked out against `escrow_ua`.
fn save_escrow_view(
    store: &Keystore,
    code: &str,
    fvk: &orchard::keys::FullViewingKey,
    network: zcash_protocol::consensus::NetworkType,
    escrow_ua: &str,
    birthday_height: Option<u32>,
) -> anyhow::Result<()> {
    if store.read(code, ESCROW_VIEW)?.is_some() {
        return Ok(());
    }
    let saved = SavedEscrowView {
        fvk: hex::encode(fvk.to_bytes()),
        network: network_name(network).to_owned(),
        escrow_ua: escrow_ua.to_owned(),
        birthday_height,
    };
    store.write(code, ESCROW_VIEW, &serde_json::to_vec_pretty(&saved)?)
}

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
/// address of a wallet it controls alone fails here. Once it matches, the
/// escrow's viewing key is kept (`escrow-view.json`, 0600) for backups and so
/// two trustees can spend without the coordinator.
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
    let network = [NetworkType::Main, NetworkType::Test, NetworkType::Regtest]
        .into_iter()
        .find(|n| address::escrow_address(&fvk, &EncodingParams(*n)).is_ok_and(|ua| ua == shown));
    let Some(network) = network else {
        bail!(
        "ESCROW ADDRESS MISMATCH for auction {code}: the address the coordinator shows ({shown}) \
         does not come from the trustees' group key. Do not send funds to it, don't let the \
         auction open, and tell the organiser and the other trustees now."
        );
    };
    let signature = sign_escrow_attestation(&identity.signing_key()?, code, state.my_slot, shown);
    // Remember what we signed, so later commands can check the public page
    // shows bidders this same address (`check_public`).
    store.write(code, ESCROW_ATTESTED, shown.as_bytes())?;
    save_escrow_view(
        store,
        code,
        &fvk,
        network,
        shown,
        state.escrow_birthday_height,
    )?;
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
        /// Every trustee's key, as listed on the public page and checked
        /// against its escrow attestation.
        keys: BTreeMap<u16, [u8; 32]>,
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
/// Trustee keys (by slot) listed on a public auction page.
fn public_keys(view: &sealforge_api::types::AuctionView) -> BTreeMap<u16, [u8; 32]> {
    view.trustees
        .iter()
        .filter_map(|t| {
            let key = hex::decode(t.pubkey.as_deref()?).ok()?.try_into().ok()?;
            Some((t.slot, key))
        })
        .collect()
}

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
            .problems
            .iter()
            .cloned()
            .chain(
                check
                    .trustees
                    .iter()
                    .filter(|t| t.verdict != sealforge_auditor::TrusteeVerdict::Signed)
                    .map(|t| format!("slot {} ({}): {:?}", t.slot, t.label, t.verdict)),
            )
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
        keys: public_keys(&view),
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

use sealforge_api::types::{BatchView, PostCommitments, PostInputs, PostShares};
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
    /// This trustee stated its round 2 inputs; its co-signer hasn't yet. No
    /// share leaves until both statements match.
    WaitingForCoSignerCheck {
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
            Self::WaitingForCoSignerCheck { batch } => write!(
                f,
                "Batch {}: you stated what you'll sign; waiting for your co-signer to do the same \
                 before sending your share. Run again shortly.",
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

/// Check the co-signer's signed statement of its round 2 inputs against this
/// trustee's own digest, with the co-signer's key from the public page. A
/// mismatch means the coordinator showed the two signers different
/// commitment sets (or altered one), so no share may leave.
fn check_co_signer_inputs(
    code: &str,
    batch: &BatchView,
    co_signer: u16,
    statement: &sealforge_api::types::SignedInputs,
    key: Option<&[u8; 32]>,
    mine: &str,
) -> anyhow::Result<()> {
    let refuse = |why: String| {
        anyhow!(
            "REFUSED batch {} (equivocation): {why} Don't sign; tell the organiser and the other \
             trustees, and treat the coordinator as compromised.",
            batch.id
        )
    };
    let key = key.ok_or_else(|| {
        refuse(format!(
            "slot {co_signer} has no key on the public auction page."
        ))
    })?;
    let digest: [u8; 32] = hex::decode(&statement.digest)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| refuse(format!("slot {co_signer}'s statement is malformed.")))?;
    let signature: [u8; 64] = hex::decode(&statement.signature)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| refuse(format!("slot {co_signer}'s statement is malformed.")))?;
    if !sealforge_api::auth::verify_round2_inputs(
        key, &signature, code, &batch.id, co_signer, &digest,
    ) {
        return Err(refuse(format!(
            "slot {co_signer}'s statement doesn't verify under its public key."
        )));
    }
    if statement.digest != mine {
        return Err(refuse(format!(
            "slot {co_signer} was shown different round 2 inputs (commitments, sighash or \
             randomizers) than you were."
        )));
    }
    Ok(())
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
/// What `trustee sign` accepts before co-signing. The default refuses anything
/// short of a public page showing the address this trustee attested, and any
/// batch whose transaction it hasn't decoded and checked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignPolicy {
    /// A lightwallet server this trustee chose (gRPC, e.g. `https://zec.rocks:443`).
    /// Refund batches are checked against the chain through it: each note's
    /// bid, its refund address, and whether an earlier batch was mined. Without
    /// it, refund batches are refused.
    pub lightwalletd: Option<String>,
    /// Sign for an auction this trustee never attested (one from before escrow
    /// attestations existed). Only the trustee can choose this, on its own
    /// command line; nothing the coordinator sends turns it on. Such an auction
    /// has no sealed viewing key, so its batches are signed without the
    /// transaction checks, and the CLI says so.
    pub allow_unattested_escrow: bool,
    /// Tests only: sign batches that carry no transaction, as the protocol
    /// tests' fake batches do. The CLI never sets it.
    #[doc(hidden)]
    pub skip_transaction_checks_for_tests: bool,
}

/// The escrow's full viewing key and network, rebuilt by this trustee from its
/// own key share and the sealed `nk ‖ rivk`, and tied to the address it
/// attested. What batch checks decrypt outputs with.
pub struct EscrowView {
    pub fvk: orchard::keys::FullViewingKey,
    pub network: zcash_protocol::consensus::NetworkType,
}

/// Rebuild [`EscrowView`]. Refuses unless the key gives exactly the address
/// this trustee attested: a coordinator can't swap in another viewing key now.
pub async fn escrow_view(
    store: &Keystore,
    client: &Client,
    code: &str,
) -> anyhow::Result<EscrowView> {
    use sealforge_escrow::address::{self, EncodingParams};
    use zcash_protocol::consensus::NetworkType;

    let attested = store
        .read(code, ESCROW_ATTESTED)?
        .ok_or_else(|| anyhow!("no record of the escrow address you attested for {code}"))?;
    let attested = String::from_utf8(attested)?;
    let (_, pubkeys) = key_share(store, code)?;
    let ak = dkg::group_key_bytes(&pubkeys)?;
    // The copy this trustee kept when it attested, checked again.
    if let Some(bytes) = store.read(code, ESCROW_VIEW)? {
        let saved: SavedEscrowView = serde_json::from_slice(&bytes)?;
        let fvk_bytes: [u8; 96] = hex::decode(&saved.fvk)?
            .try_into()
            .map_err(|_| anyhow!("{ESCROW_VIEW} for {code} is malformed"))?;
        let fvk = orchard::keys::FullViewingKey::from_bytes(&fvk_bytes)
            .ok_or_else(|| anyhow!("{ESCROW_VIEW} for {code} holds no valid viewing key"))?;
        let network = network_from_name(&saved.network)?;
        ensure!(
            fvk_bytes[..32] == ak
                && address::escrow_address(&fvk, &EncodingParams(network))
                    .is_ok_and(|ua| ua == attested),
            "{ESCROW_VIEW} for {code} doesn't give the address you attested. Don't sign; check \
             this machine."
        );
        return Ok(EscrowView { fvk, network });
    }
    let state: DkgState = client.get(&format!("/v1/auctions/{code}/dkg")).await?;
    let sealed = hex::decode(
        state
            .escrow_view_secret_for_me
            .ok_or_else(|| anyhow!("the coordinator sent no escrow view secret for {code}"))?,
    )?;
    let identity = store.trustee_identity(code)?;
    let view_secret: [u8; 64] = identity
        .enc_secret()?
        .unseal(&sealed)
        .map_err(|_| anyhow!("could not open the escrow view secret"))?
        .try_into()
        .map_err(|_| anyhow!("the escrow view secret has the wrong length"))?;
    let fvk = address::escrow_fvk_from_parts(&ak, &view_secret)?;
    let network = [NetworkType::Main, NetworkType::Test, NetworkType::Regtest]
        .into_iter()
        .find(|n| address::escrow_address(&fvk, &EncodingParams(*n)).is_ok_and(|ua| ua == attested))
        .ok_or_else(|| {
            anyhow!(
                "ESCROW KEY MISMATCH for {code}: the viewing key the coordinator sent doesn't give \
                 the address you attested. Don't sign; tell the organiser and the other trustees."
            )
        })?;
    // Keystores from before escrow views were kept get one now.
    save_escrow_view(
        store,
        code,
        &fvk,
        network,
        &attested,
        state.escrow_birthday_height,
    )?;
    Ok(EscrowView { fvk, network })
}

/// This trustee's backup string for `code`: its signing share and the
/// escrow's viewing key ([`sealforge_escrow::backup`]). Secret. A keystore
/// from before viewing keys were kept fetches it from the coordinator once,
/// checked against the address this trustee attested.
pub async fn backup(store: &Keystore, coordinator: &str, code: &str) -> anyhow::Result<String> {
    let (key_package, pubkeys) = key_share(store, code)?;
    if store.read(code, ESCROW_VIEW)?.is_none() {
        let identity = store.trustee_identity(code)?;
        escrow_view(
            store,
            &Client::new(coordinator, Some(identity.signing_key()?)),
            code,
        )
        .await?;
    }
    let saved: SavedEscrowView = serde_json::from_slice(
        &store
            .read(code, ESCROW_VIEW)?
            .ok_or_else(|| anyhow!("no escrow viewing key kept for {code}"))?,
    )?;
    let fvk_bytes: [u8; 96] = hex::decode(&saved.fvk)?
        .try_into()
        .map_err(|_| anyhow!("{ESCROW_VIEW} for {code} is malformed"))?;
    let fvk = orchard::keys::FullViewingKey::from_bytes(&fvk_bytes)
        .ok_or_else(|| anyhow!("{ESCROW_VIEW} for {code} holds no valid viewing key"))?;
    let n = u16::try_from(pubkeys.verifying_shares().len())?;
    let backup = sealforge_escrow::backup::Backup::new(
        &key_package,
        n,
        &fvk,
        network_from_name(&saved.network)?,
        saved.birthday_height.unwrap_or(0),
    )?;
    Ok(backup.encode()?)
}

/// What a backup string restores, checked against this machine's keystore.
#[derive(Debug, PartialEq, Eq)]
pub struct BackupCheck {
    pub slot: u16,
    pub threshold: (u8, u8),
    pub escrow_ua: String,
    pub birthday_height: u32,
}

/// Check a backup string for `code` against this keystore: the same trustee,
/// the same signing share, the same group key, and a viewing key that gives
/// the address this trustee attested. Proves the string restores what this
/// machine holds, without writing anything.
pub fn check_backup(store: &Keystore, code: &str, text: &str) -> anyhow::Result<BackupCheck> {
    use sealforge_escrow::address::{self, EncodingParams};

    ensure!(
        !text.trim().is_empty(),
        "no backup string given (the input is empty)"
    );
    let backup = sealforge_escrow::backup::Backup::decode(text)?;
    let restored = backup.key_package()?;
    let (key_package, _) = key_share(store, code)?;
    ensure!(
        restored == key_package,
        "this backup's key share isn't the one this machine holds for {code} (another trustee's, \
         or another auction's)"
    );
    let attested = String::from_utf8(
        store
            .read(code, ESCROW_ATTESTED)?
            .ok_or_else(|| anyhow!("no record of the escrow address you attested for {code}"))?,
    )?;
    let escrow_ua =
        address::escrow_address(&backup.full_viewing_key(), &EncodingParams(backup.network))?;
    ensure!(
        escrow_ua == attested,
        "this backup's viewing key gives {escrow_ua}, not the address you attested ({attested})"
    );
    Ok(BackupCheck {
        slot: backup.id as u16,
        threshold: (backup.t, backup.n),
        escrow_ua,
        birthday_height: backup.birthday_height,
    })
}

/// Decode and check a batch's transaction before taking part in signing it
/// (security review D1): the shared co-signer checks (frost-zec spec § 6)
/// and sealForge's rule that every payment is owed. Returns what it checked.
/// A batch this trustee checked: what the transaction does, and for refund
/// batches the ledger entry to record before its shares leave this machine.
pub struct CheckedBatch {
    pub verified: sealforge_verify::VerifiedBatch,
    pub ledger_entry: Option<sealforge_verify::refunds::LedgerEntry>,
}

/// Refund batches this trustee signed, kept so it never signs a note twice.
const REFUND_LEDGER: &str = "refund-ledger.json";

fn read_ledger(
    store: &Keystore,
    code: &str,
) -> anyhow::Result<Vec<sealforge_verify::refunds::LedgerEntry>> {
    Ok(match store.read(code, REFUND_LEDGER)? {
        Some(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("reading {REFUND_LEDGER} for {code}"))?,
        None => Vec::new(),
    })
}

/// Record a signed refund batch (synced to disk) before its shares are sent.
fn record_signed(
    store: &Keystore,
    code: &str,
    entry: sealforge_verify::refunds::LedgerEntry,
) -> anyhow::Result<()> {
    let mut ledger = read_ledger(store, code)?;
    if !ledger.iter().any(|e| e.batch_txid == entry.batch_txid) {
        ledger.push(entry);
        store.replace(code, REFUND_LEDGER, &serde_json::to_vec_pretty(&ledger)?)?;
    }
    Ok(())
}

fn chain_params(network: zcash_protocol::consensus::NetworkType) -> sealforge_chain::ChainParams {
    use zcash_protocol::consensus::NetworkType;
    match network {
        NetworkType::Main => sealforge_chain::ChainParams::Mainnet,
        NetworkType::Test => sealforge_chain::ChainParams::Testnet,
        NetworkType::Regtest => sealforge_chain::ChainParams::regtest(),
    }
}

/// Tie every note of a refund batch to its bid, read from the chain through
/// the trustee's own lightwallet server, and to what this trustee signed
/// before (security review D1, second part).
#[allow(clippy::too_many_arguments)]
async fn check_refund_recipients(
    store: &Keystore,
    code: &str,
    view: &EscrowView,
    batch: &BatchView,
    pczt: &[u8],
    verified: &sealforge_verify::VerifiedBatch,
    result: &sealforge_api::types::PublishedResult,
    chain: &mut sealforge_chain::Client,
) -> anyhow::Result<sealforge_verify::refunds::LedgerEntry> {
    use sealforge_verify::refunds::{self, BidOnChain, RefundClaim, ResultBid};

    let claims = batch
        .refunds
        .iter()
        .map(|n| {
            Ok(RefundClaim {
                bid_txid: hex32(&n.bid_txid)?,
                bid_output_index: n.bid_output_index,
                value_zat: n.value_zat,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Each claimed bid, as this trustee reads it from the chain.
    let scanner = sealforge_scan::EscrowScanner::from_fvk(chain_params(view.network), &view.fvk);
    let mut bids = BTreeMap::new();
    for claim in &claims {
        if bids.contains_key(&claim.bid()) {
            continue;
        }
        let Some((raw, Some(height))) =
            sealforge_chain::fetch_transaction(chain, claim.bid_txid).await?
        else {
            continue; // Not mined (or unknown): the check refuses this note.
        };
        let payments = scanner
            .decrypt_transaction(&raw, height)
            .map_err(|e| anyhow!("decrypting bid {}: {e}", hex::encode(claim.bid_txid)))?;
        if let Some(p) = payments
            .into_iter()
            .find(|p| p.txid == claim.bid_txid && p.output_index == claim.bid_output_index)
        {
            let memo = p
                .memo
                .as_deref()
                .and_then(|m| sealforge_core::memo::BidMemo::decode(m).ok());
            bids.insert(
                claim.bid(),
                BidOnChain {
                    amount_zat: p.amount_zat,
                    refund_ua: memo.as_ref().map(|m| m.refund_ua.clone()),
                    commitment: memo.map(|m| m.commitment).unwrap_or_default(),
                },
            );
        }
    }

    let listed = result
        .bids
        .iter()
        .map(|b| {
            Ok((
                (hex32(&b.txid)?, b.output_index),
                ResultBid {
                    commitment: hex32(&b.commitment)?,
                    refund_due_zat: b.refund_due_zat,
                },
            ))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;

    // What this trustee signed before, from batches that are or may still be
    // on chain.
    let tip = sealforge_chain::chain_tip(chain).await?;
    let mut entries = Vec::new();
    for entry in read_ledger(store, code)? {
        let on_chain = sealforge_chain::fetch_transaction(chain, entry.batch_txid)
            .await?
            .is_some();
        entries.push((entry, on_chain));
    }
    let batch_txid = sealforge_verify::batch_txid(pczt).map_err(|r| anyhow!("{r}"))?;
    let before = refunds::signed_before(&entries, tip, &batch_txid);

    refunds::check_refund_recipients(
        &verified.payments,
        &claims,
        &bids,
        &listed,
        &before,
        sealforge_verify::orchard_receiver,
    )
    .map_err(|refusal| {
        anyhow!(
            "REFUSED batch {} ({}): {}. Don't sign; tell the organiser and the other trustees.",
            batch.id,
            refusal.class,
            refusal.message
        )
    })?;
    Ok(refunds::LedgerEntry {
        batch_txid,
        expiry_height: verified.expiry_height,
        notes: claims,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn check_batch(
    store: &Keystore,
    coordinator: &str,
    code: &str,
    view: &EscrowView,
    batch: &BatchView,
    chain: Option<&mut sealforge_chain::Client>,
) -> anyhow::Result<CheckedBatch> {
    use sealforge_verify::{BatchKind, BatchRules};

    let pczt = hex::decode(batch.pczt.as_deref().ok_or_else(|| {
        anyhow!(
            "REFUSED batch {}: the coordinator sent no transaction to check",
            batch.id
        )
    })?)?;
    let kind = BatchKind::parse(&batch.kind).ok_or_else(|| {
        anyhow!(
            "REFUSED batch {}: unknown batch kind {}",
            batch.id,
            batch.kind
        )
    })?;
    let result: Option<sealforge_api::types::PublishedResult> = if kind == BatchKind::Refund {
        Some(
            Client::new(coordinator, None)
                .get(&format!("/v1/auctions/{code}/result"))
                .await
                .with_context(|| {
                    format!(
                        "REFUSED batch {}: no published result to check refunds against",
                        batch.id
                    )
                })?,
        )
    } else {
        None
    };
    let refund_total_due_zat = result
        .as_ref()
        .map(|r| r.bids.iter().map(|b| b.refund_due_zat).sum());
    let rules = BatchRules {
        kind,
        refund_total_due_zat,
        payout_ua: batch.payout_ua.clone(),
    };
    let spends = batch
        .spends
        .iter()
        .map(|s| Ok((s.action_index as usize, hex32(&s.alpha)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let verified = sealforge_verify::verify_batch(
        &pczt,
        &view.fvk,
        view.network,
        now,
        &hex32(&batch.sighash)?,
        &spends,
        &rules,
    )
    .map_err(|refusal| {
        anyhow!(
            "REFUSED batch {} ({}): {}. Don't sign; tell the organiser and the other trustees.",
            batch.id,
            refusal.class,
            refusal.message
        )
    })?;
    let ledger_entry = match result {
        Some(result) => {
            let chain = chain.ok_or_else(|| {
                anyhow!(
                    "REFUSED batch {}: refund batches are checked against the chain; pass \
                     --lightwalletd with a lightwallet server you trust",
                    batch.id
                )
            })?;
            Some(
                check_refund_recipients(store, code, view, batch, &pczt, &verified, &result, chain)
                    .await?,
            )
        }
        None => None,
    };
    Ok(CheckedBatch {
        verified,
        ledger_entry,
    })
}

/// Before co-signing money out of an escrow: the public page bidders paid from
/// must be up and show the address this trustee attested, its own key, and
/// valid attestations (security review F15). Anything else refuses.
pub async fn require_public(
    store: &Keystore,
    coordinator: &str,
    code: &str,
    policy: &SignPolicy,
) -> anyhow::Result<BTreeMap<u16, [u8; 32]>> {
    match check_public(store, coordinator, code).await? {
        PublicCheck::Matches { keys, .. } => Ok(keys),
        PublicCheck::NotPublicYet => bail!(
            "Refusing to co-sign for {code}: the public auction page isn't available, so you \
             can't confirm it shows bidders the escrow address you attested."
        ),
        // No attestations to check the keys against: take the listed ones.
        PublicCheck::NotAttested if policy.allow_unattested_escrow => {
            let view: sealforge_api::types::AuctionView = Client::new(coordinator, None)
                .get(&format!("/v1/auctions/{code}"))
                .await?;
            Ok(public_keys(&view))
        }
        PublicCheck::NotAttested => bail!(
            "Refusing to co-sign for {code}: this keystore has no record of the escrow address \
             you attested. Run `sealforge trustee dkg {code}` to check it. For an auction from \
             before escrow attestations, pass --allow-unattested-escrow."
        ),
    }
}

pub async fn sign_step(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<Vec<SignProgress>> {
    sign_step_with(store, coordinator, code, SignPolicy::default()).await
}

pub async fn sign_step_with(
    store: &Keystore,
    coordinator: &str,
    code: &str,
    policy: SignPolicy,
) -> anyhow::Result<Vec<SignProgress>> {
    let identity = store.trustee_identity(code)?;
    let (key_package, _) = key_share(store, code)?;
    // Never co-sign money out of an escrow whose public page showed bidders
    // something other than what this trustee verified, or that is missing.
    let public_keys = require_public(store, coordinator, code, &policy).await?;
    let client = Client::new(coordinator, Some(identity.signing_key()?));
    let batches: Vec<BatchView> = client.get(&format!("/v1/auctions/{code}/batches")).await?;
    let me = identity.slot;
    let mut out = Vec::new();
    // The escrow's viewing key, rebuilt once and only when there is something
    // to check. Auctions from before attestations have none.
    let mut view: Option<EscrowView> = None;
    let checks = !policy.skip_transaction_checks_for_tests;
    // The trustee's own lightwallet server, connected on first use.
    let mut chain: Option<sealforge_chain::Client> = None;

    for batch in batches {
        if batch.status != "awaiting_signatures" {
            out.push(SignProgress::Done { batch: batch.id });
            continue;
        }
        let mut checked: Option<CheckedBatch> = None;
        // Check the transaction before committing nonces and before signing.
        let will_act = !batch.i_committed
            || (batch.signer_slots.contains(&me)
                && !batch.i_shared
                && batch.signer_commitments.len() >= 2);
        if checks && will_act && store.read(code, &signed_file(&batch.id))?.is_none() {
            if view.is_none() {
                view = match escrow_view(store, &client, code).await {
                    Ok(v) => Some(v),
                    Err(e)
                        if policy.allow_unattested_escrow
                            && store.read(code, ESCROW_ATTESTED)?.is_none() =>
                    {
                        eprintln!(
                            "WARNING: {code} has no attested escrow, so batch {} is signed without \
                             checking its transaction ({e}).",
                            batch.id
                        );
                        None
                    }
                    Err(e) => return Err(e),
                };
            }
            if let Some(view) = &view {
                if chain.is_none() && batch.kind == "refund" {
                    if let Some(url) = &policy.lightwalletd {
                        chain = Some(sealforge_chain::connect(url).await.with_context(|| {
                            format!("connecting to the lightwallet server {url}")
                        })?);
                    }
                }
                checked = Some(
                    check_batch(store, coordinator, code, view, &batch, chain.as_mut()).await?,
                );
            }
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
        let digest = sealforge_api::auth::round2_inputs_digest(&sighash, &alphas, &commitments);
        let inputs = hex::encode(digest);

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

        // Between rounds: state what this trustee will sign over, and sign only
        // once the co-signer has stated the same (stream 2c).
        match batch.signer_inputs.get(&me) {
            None => {
                let signature = sealforge_api::auth::sign_round2_inputs(
                    &identity.signing_key()?,
                    code,
                    &batch.id,
                    me,
                    &digest,
                );
                let _: BatchView = client
                    .post(
                        &format!("/v1/batches/{}/inputs", batch.id),
                        &PostInputs {
                            digest: inputs.clone(),
                            signature: hex::encode(signature),
                        },
                    )
                    .await?;
            }
            Some(stated) if stated.digest != inputs => bail!(
                "REFUSED batch {} (equivocation): the coordinator now shows different round 2 \
                 inputs than the ones you stated. Don't sign; tell the organiser and the other \
                 trustees, and treat the coordinator as compromised.",
                batch.id
            ),
            Some(_) => {}
        }
        let co_signer = batch
            .signer_slots
            .iter()
            .copied()
            .find(|s| *s != me)
            .ok_or_else(|| anyhow!("batch {} has no co-signer", batch.id))?;
        let Some(theirs) = batch.signer_inputs.get(&co_signer) else {
            out.push(SignProgress::WaitingForCoSignerCheck { batch: batch.id });
            continue;
        };
        check_co_signer_inputs(
            code,
            &batch,
            co_signer,
            theirs,
            public_keys.get(&co_signer),
            &inputs,
        )?;

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
        // A refund batch goes in this trustee's ledger before its shares can
        // leave, so it never signs the same notes in another batch (D1).
        if let Some(entry) = checked.and_then(|c| c.ledger_entry) {
            record_signed(store, code, entry)?;
        }
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
