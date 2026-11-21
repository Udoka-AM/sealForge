//! Spend from an escrow with two trustees' backup strings and no coordinator
//! (issue #64). If the coordinator and its database are gone, two trustees
//! can still move the escrowed funds to an address they agree on, using only
//! their backups ([`sealforge_escrow::backup`]) and a lightwallet server.
//!
//! The session is two files passed between the trustees out of band, in the
//! order the frost-zec spec (§ 5) gives for a coordinator and a co-signer:
//!
//! 1. [`start`] (trustee A): rebuilds the escrow wallet from A's backup,
//!    scans it from the birthday height, builds and proves a sweep of every
//!    note to the agreed address, commits fresh nonces, and writes a
//!    [`Request`]. A's nonces stay on A's machine (owner-only file).
//! 2. [`cosign`] (trustee B): checks the transaction from B's own backup with
//!    the same rules as a payout (one output, to the address B was told out of
//!    band, no hidden outputs or inflated fee), and only when B approves,
//!    commits and signs in one step and writes a [`Reply`]. B's nonces never
//!    leave memory.
//! 3. [`finish`] (trustee A): signs over both commitment sets, deleting its
//!    nonces first, aggregates, checks every signature against the spend's key
//!    while applying it, and broadcasts.
//!
//! One reply per request: A's nonces are gone after [`finish`], so the same
//! request can't be signed twice.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, ensure, Context};
use sealforge_chain::{
    chain_tip, connect,
    spend::{apply_signatures, pczt_from_bytes, pczt_to_bytes, send_raw, MAX_NOTES_PER_TX},
    ChainParams, EscrowWallet,
};
use sealforge_escrow::{
    address::{escrow_address, escrow_view_keys, EncodingParams},
    backup::Backup,
    dkg,
    frost::{
        keys::{PublicKeyPackage, VerifyingShare},
        round1::SigningCommitments,
        round2::SignatureShare,
        VerifyingKey,
    },
    signing::{self, SpendAuthRequest},
};
use serde::{Deserialize, Serialize};
use zcash_protocol::consensus::NetworkType;

const VERSION: &str = "sealforge-recovery-v1";

/// Trustee A → trustee B: the transaction to sign and A's commitments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub version: String,
    pub network: String,
    pub escrow_ua: String,
    /// The address every note is swept to, agreed out of band.
    pub to: String,
    pub amount_zat: u64,
    /// The proved, unsigned transaction (PCZT, hex).
    pub pczt: String,
    pub sighash: String,
    /// One per escrow spend: `(action index, alpha hex)`.
    pub spends: Vec<(usize, String)>,
    pub initiator_slot: u16,
    /// A's commitment per spend (hex).
    pub initiator_commitments: Vec<String>,
}

/// Trustee B → trustee A: B's commitments and signature shares.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reply {
    pub version: String,
    pub sighash: String,
    pub cosigner_slot: u16,
    /// B's verifying share (public), so A can aggregate and check B's shares.
    pub cosigner_verifying_share: String,
    pub cosigner_commitments: Vec<String>,
    pub cosigner_shares: Vec<String>,
}

/// What A keeps between [`start`] and [`finish`] (owner-only).
#[derive(Serialize, Deserialize)]
struct Secret {
    sighash: String,
    nonces: Vec<String>,
}

fn network_name(n: NetworkType) -> &'static str {
    match n {
        NetworkType::Main => "mainnet",
        NetworkType::Test => "testnet",
        NetworkType::Regtest => "regtest",
    }
}

fn chain_params(n: NetworkType) -> ChainParams {
    match n {
        NetworkType::Main => ChainParams::Mainnet,
        NetworkType::Test => ChainParams::Testnet,
        NetworkType::Regtest => ChainParams::regtest(),
    }
}

fn hex32(value: &str) -> anyhow::Result<[u8; 32]> {
    hex::decode(value)?
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes of hex"))
}

fn read_backup(path: &Path) -> anyhow::Result<Backup> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the backup {}", path.display()))?;
    Ok(Backup::decode(&text)?)
}

/// The escrow address a backup's viewing key gives.
fn backup_address(backup: &Backup) -> anyhow::Result<String> {
    Ok(escrow_address(
        &backup.full_viewing_key(),
        &EncodingParams(backup.network),
    )?)
}

fn secret_path(work: &Path) -> PathBuf {
    work.join("nonces.secret.json")
}

/// Where [`start`] keeps the escrow wallet and its nonces for this escrow.
pub fn default_work_dir(root: &Path, backup: &Path) -> anyhow::Result<PathBuf> {
    let address = backup_address(&read_backup(backup)?)?;
    Ok(root.join("recovery").join(&address[..24]))
}

/// What [`start`] built.
pub struct Started {
    pub request: Request,
    pub notes: usize,
    pub more_left: bool,
}

/// Step 1 (trustee A). `birthday` overrides the backup's birthday height.
pub async fn start(
    backup_file: &Path,
    lightwalletd: &str,
    to: &str,
    work: &Path,
    birthday: Option<u32>,
) -> anyhow::Result<Started> {
    let backup = read_backup(backup_file)?;
    ensure!(
        sealforge_verify::orchard_receiver(to).is_some(),
        "{to} has no Orchard receiver; sweep to a unified address with one"
    );
    let key_package = backup.key_package()?;
    let params = chain_params(backup.network);
    let (ufvk, _) = escrow_view_keys(&backup.full_viewing_key(), &params)?;
    let escrow_ua = backup_address(&backup)?;

    std::fs::create_dir_all(work)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(work, std::fs::Permissions::from_mode(0o700))?;
    }
    ensure!(
        !secret_path(work).exists(),
        "a recovery session is already open in {} (its nonces are waiting for a reply). Finish \
         it, or delete that directory to start over.",
        work.display()
    );
    let mut client = connect(lightwalletd)
        .await
        .with_context(|| format!("connecting to {lightwalletd}"))?;
    let wallet_path = work.join("escrow.sqlite");
    let mut wallet = if wallet_path.exists() {
        EscrowWallet::open(&wallet_path, params)?
    } else {
        let height = match birthday.unwrap_or(backup.birthday_height) {
            0 => bail!(
                "this backup doesn't record the escrow's birthday height; pass --birthday (the \
                 height the auction's escrow was created at, or earlier)"
            ),
            h => h,
        };
        EscrowWallet::create(&wallet_path, params, &ufvk, height, &mut client).await?
    };
    wallet.sync(&mut client).await?;
    let sweep = tokio::task::block_in_place(|| wallet.build_sweep(to, 0, MAX_NOTES_PER_TX, None))?;
    let unsigned = sweep.payment;

    let round1 =
        signing::commit_batch(&key_package, unsigned.spends.len(), &mut rand::rngs::OsRng)?;
    let secret = Secret {
        sighash: hex::encode(unsigned.sighash),
        nonces: round1.nonces.iter().map(hex::encode).collect(),
    };
    crate::keystore::write_private(&secret_path(work), &serde_json::to_vec(&secret)?)?;

    let request = Request {
        version: VERSION.into(),
        network: network_name(backup.network).into(),
        escrow_ua,
        to: to.into(),
        amount_zat: sweep.amount_zat,
        sighash: hex::encode(unsigned.sighash),
        spends: unsigned
            .spends
            .iter()
            .map(|s| (s.action_index, hex::encode(s.alpha)))
            .collect(),
        pczt: hex::encode(pczt_to_bytes(unsigned.pczt)?),
        initiator_slot: backup.id as u16,
        initiator_commitments: round1.commitments.iter().map(hex::encode).collect(),
    };
    std::fs::write(
        work.join("request.json"),
        serde_json::to_vec_pretty(&request)?,
    )?;
    Ok(Started {
        request,
        notes: sweep.notes,
        more_left: sweep.more_left,
    })
}

/// What [`cosign`] checked in the transaction (shown before approval).
pub struct Reviewed {
    pub verified: sealforge_verify::VerifiedBatch,
    pub reply: Option<Reply>,
}

/// Step 2 (trustee B). `to` is the address B agreed to, typed by B, not read
/// from the request. Without `approve`, only checks and shows the transaction.
pub fn cosign(
    backup_file: &Path,
    request: &Request,
    to: &str,
    approve: bool,
) -> anyhow::Result<Reviewed> {
    use sealforge_verify::{BatchKind, BatchRules};

    let backup = read_backup(backup_file)?;
    let refuse = |why: String| anyhow!("REFUSED: {why} Don't sign; talk to the other trustee.");
    ensure!(
        request.version == VERSION,
        "unknown request version {}",
        request.version
    );
    if request.network != network_name(backup.network) {
        return Err(refuse(format!(
            "the request is for {}, your backup for {}.",
            request.network,
            network_name(backup.network)
        )));
    }
    let escrow_ua = backup_address(&backup)?;
    if request.escrow_ua != escrow_ua {
        return Err(refuse(format!(
            "the request spends from {}, but your backup's escrow is {escrow_ua}.",
            request.escrow_ua
        )));
    }
    let me = backup.id as u16;
    if request.initiator_slot == me
        || request.initiator_slot == 0
        || request.initiator_slot > backup.n as u16
    {
        return Err(refuse(format!(
            "the request names trustee {} as the other signer; you are trustee {me}.",
            request.initiator_slot
        )));
    }
    if sealforge_verify::orchard_receiver(&request.to) != sealforge_verify::orchard_receiver(to) {
        return Err(refuse(format!(
            "the request sweeps to {}, not the address you agreed ({to}).",
            request.to
        )));
    }
    let pczt = hex::decode(&request.pczt)?;
    let spends = request
        .spends
        .iter()
        .map(|(i, a)| Ok((*i, hex32(a)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(
        request.initiator_commitments.len() == spends.len(),
        "the request has {} commitments for {} spends",
        request.initiator_commitments.len(),
        spends.len()
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    // The same checks as a payout: one output, to the agreed address.
    let verified = sealforge_verify::verify_batch(
        &pczt,
        &backup.full_viewing_key(),
        backup.network,
        now,
        &hex32(&request.sighash)?,
        &spends,
        &BatchRules {
            kind: BatchKind::Payout,
            refund_total_due_zat: None,
            payout_ua: Some(to.to_owned()),
            fee: None,
            fee_already_signed: false,
        },
    )
    .map_err(|r| refuse(format!("({}) {}.", r.class, r.message)))?;
    if !approve {
        return Ok(Reviewed {
            verified,
            reply: None,
        });
    }

    // Commit last and sign at once, over the sighash computed here.
    let key_package = backup.key_package()?;
    let round1 = signing::commit_batch(&key_package, spends.len(), &mut rand::rngs::OsRng)?;
    let mut commitments = BTreeMap::new();
    commitments.insert(
        request.initiator_slot,
        request
            .initiator_commitments
            .iter()
            .map(hex::decode)
            .collect::<Result<Vec<_>, _>>()?,
    );
    commitments.insert(me, round1.commitments.clone());
    let alphas: Vec<[u8; 32]> = spends.iter().map(|(_, a)| *a).collect();
    let shares = signing::sign_batch(
        &key_package,
        verified.sighash,
        &alphas,
        &commitments,
        &round1.nonces,
    )?;
    drop(round1.nonces);
    Ok(Reviewed {
        verified: verified.clone(),
        reply: Some(Reply {
            version: VERSION.into(),
            sighash: hex::encode(verified.sighash),
            cosigner_slot: me,
            cosigner_verifying_share: hex::encode(key_package.verifying_share().serialize()?),
            cosigner_commitments: round1.commitments.iter().map(hex::encode).collect(),
            cosigner_shares: shares.iter().map(hex::encode).collect(),
        }),
    })
}

/// Step 3 (trustee A): sign, aggregate, broadcast. Returns the txid.
pub async fn finish(
    backup_file: &Path,
    work: &Path,
    reply: &Reply,
    lightwalletd: &str,
) -> anyhow::Result<String> {
    let backup = read_backup(backup_file)?;
    let key_package = backup.key_package()?;
    let me = backup.id as u16;
    let request: Request = serde_json::from_slice(
        &std::fs::read(work.join("request.json")).context("no request.json: run recover-start")?,
    )?;
    let secret_file = secret_path(work);
    let secret: Secret =
        serde_json::from_slice(&std::fs::read(&secret_file).with_context(|| {
            format!(
                "no open session in {} (its nonces were already used, or it never started)",
                work.display()
            )
        })?)?;
    ensure!(
        reply.version == VERSION
            && reply.sighash == request.sighash
            && secret.sighash == request.sighash,
        "the reply is for another transaction"
    );
    ensure!(
        reply.cosigner_slot != me,
        "the reply comes from your own slot"
    );
    let n = request.spends.len();
    ensure!(
        reply.cosigner_commitments.len() == n && reply.cosigner_shares.len() == n,
        "the reply has the wrong number of commitments or shares"
    );
    let sighash = hex32(&request.sighash)?;
    let alphas: Vec<[u8; 32]> = request
        .spends
        .iter()
        .map(|(_, a)| hex32(a))
        .collect::<anyhow::Result<_>>()?;
    let decode = |v: &[String]| v.iter().map(hex::decode).collect::<Result<Vec<_>, _>>();
    let mut commitments = BTreeMap::new();
    commitments.insert(me, decode(&request.initiator_commitments)?);
    commitments.insert(reply.cosigner_slot, decode(&reply.cosigner_commitments)?);
    let nonces = decode(&secret.nonces)?;
    // The nonces are used once: delete them before signing with them.
    std::fs::remove_file(&secret_file)
        .with_context(|| format!("deleting used nonces {}", secret_file.display()))?;
    let mine = signing::sign_batch(&key_package, sighash, &alphas, &commitments, &nonces)?;

    let their_id = dkg::identifier(reply.cosigner_slot)?;
    let my_id = *key_package.identifier();
    let mut verifying_shares = BTreeMap::new();
    verifying_shares.insert(my_id, *key_package.verifying_share());
    verifying_shares.insert(
        their_id,
        VerifyingShare::deserialize(&hex::decode(&reply.cosigner_verifying_share)?)?,
    );
    let pubkeys = PublicKeyPackage::new(
        verifying_shares,
        VerifyingKey::deserialize(&backup.ak())?,
        Some(backup.t as u16),
    );
    let mut signatures = Vec::with_capacity(n);
    for (i, ((action_index, _), alpha)) in request.spends.iter().zip(&alphas).enumerate() {
        let request = SpendAuthRequest {
            sighash,
            alpha: *alpha,
        };
        let mut c = BTreeMap::new();
        c.insert(
            my_id,
            SigningCommitments::deserialize(&commitments[&me][i])?,
        );
        c.insert(
            their_id,
            SigningCommitments::deserialize(&commitments[&reply.cosigner_slot][i])?,
        );
        let mut s = BTreeMap::new();
        s.insert(my_id, SignatureShare::deserialize(&mine[i])?);
        s.insert(
            their_id,
            SignatureShare::deserialize(&hex::decode(&reply.cosigner_shares[i])?)?,
        );
        let package = signing::signing_package(&request, c);
        let signature = signing::aggregate(&request, &package, &s, &pubkeys)
            .context("the signatures don't aggregate: the reply's shares are wrong")?;
        signing::verify_spend_auth(&backup.ak(), &request, &signature)?;
        signatures.push((*action_index, signature));
    }
    let params = chain_params(backup.network);
    let mut wallet = EscrowWallet::open(&work.join("escrow.sqlite"), params)?;
    let pczt = apply_signatures(
        pczt_from_bytes(&hex::decode(&request.pczt)?)?,
        sighash,
        &signatures,
    )?;
    let txid = tokio::task::block_in_place(|| wallet.finalize(pczt))?;
    let raw = wallet.raw_transaction(txid)?;
    let mut client = connect(lightwalletd).await?;
    let tip = chain_tip(&mut client).await?;
    send_raw(&mut client, raw)
        .await
        .with_context(|| format!("broadcasting at tip {tip}"))?;
    Ok(txid.to_string())
}

#[cfg(test)]
mod tests {
    use rand::rngs::OsRng;
    use sealforge_escrow::{address, Threshold};

    use super::*;

    /// Backup files for trustees 1 and 2 of a fresh testnet escrow, and its address.
    fn escrow(dir: &Path) -> (PathBuf, String) {
        std::fs::create_dir_all(dir).unwrap();
        let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
        let fvk =
            address::escrow_fvk(&dkg::group_key_bytes(&pubkeys).unwrap(), &mut OsRng).unwrap();
        let b = Backup::new(
            &keys[&dkg::identifier(2).unwrap()],
            3,
            &fvk,
            NetworkType::Test,
            1,
        )
        .unwrap();
        let path = dir.join("t2.txt");
        std::fs::write(&path, b.encode().unwrap()).unwrap();
        let ua = escrow_address(&fvk, &EncodingParams(NetworkType::Test)).unwrap();
        (path, ua)
    }

    fn request(escrow_ua: &str, initiator: u16, to: &str) -> Request {
        Request {
            version: VERSION.into(),
            network: "testnet".into(),
            escrow_ua: escrow_ua.into(),
            to: to.into(),
            amount_zat: 1,
            pczt: String::new(),
            sighash: hex::encode([0u8; 32]),
            spends: vec![(0, hex::encode([0u8; 32]))],
            initiator_slot: initiator,
            initiator_commitments: vec![String::new()],
        }
    }

    #[test]
    fn the_cosigner_refuses_before_reading_the_transaction() {
        let dir = std::env::temp_dir().join(format!("sf-recover-{}", std::process::id()));
        let (b, ua) = escrow(&dir.join("a"));
        // Two valid addresses with Orchard receivers to use as destinations.
        let (_, agreed) = escrow(&dir.join("dest1"));
        let (_, other) = escrow(&dir.join("dest2"));
        let err = |r: anyhow::Result<Reviewed>| r.err().expect("refused").to_string();

        // Another escrow than the co-signer's backup.
        assert!(err(cosign(&b, &request(&other, 1, &agreed), &agreed, true))
            .contains("your backup's escrow"));
        // Another network.
        let mut wrong_net = request(&ua, 1, &agreed);
        wrong_net.network = "mainnet".into();
        assert!(err(cosign(&b, &wrong_net, &agreed, true)).contains("is for mainnet"));
        // The co-signer's own slot as the other signer.
        assert!(
            err(cosign(&b, &request(&ua, 2, &agreed), &agreed, true)).contains("you are trustee 2")
        );
        // A destination other than the one the co-signer typed.
        assert!(err(cosign(&b, &request(&ua, 1, &other), &agreed, true))
            .contains("not the address you agreed"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
