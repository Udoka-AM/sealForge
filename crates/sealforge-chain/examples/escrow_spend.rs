//! On-chain FROST 2-of-3 spend from an escrow (task 1.5's on-chain step).
//!
//! escrow_spend <escrow.sqlite> <keys-dir from `sealforge escrow local-dkg`> <to-ua> <zat> <signer-slots e.g. 1,3>

use std::collections::BTreeMap;

use rand::rngs::OsRng;
use sealforge_chain::{
    connect,
    spend::{apply_signatures, PaymentOut},
    ChainParams, EscrowWallet,
};
use sealforge_escrow::{
    dkg,
    frost::keys::{KeyPackage, PublicKeyPackage},
    signing::{self, SpendAuthRequest},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (wallet_path, keys, to) = (&args[1], std::path::PathBuf::from(&args[2]), &args[3]);
    let amount: u64 = args[4].parse()?;
    let slots: Vec<u16> = args[5]
        .split(',')
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;

    let mut client = connect("http://127.0.0.1:8137").await?;
    let mut escrow = EscrowWallet::open(wallet_path.as_ref(), ChainParams::regtest())?;
    escrow.sync(&mut client).await?;

    let started = std::time::Instant::now();
    let unsigned = escrow.build_payment(&[PaymentOut {
        address: to.clone(),
        amount_zat: amount,
        memo: Some("sealForge escrow payout (2-of-3 FROST)".into()),
    }])?;
    println!(
        "built and proved PCZT in {:.1}s: {} escrow spend(s) to authorize",
        started.elapsed().as_secs_f64(),
        unsigned.spends.len()
    );

    let read = |name: &str| -> anyhow::Result<Vec<u8>> {
        Ok(hex::decode(
            std::fs::read_to_string(keys.join(name))?.trim(),
        )?)
    };
    let pubkeys = PublicKeyPackage::deserialize(&read("public-key-package.hex")?)?;
    let trustees: BTreeMap<u16, KeyPackage> = slots
        .iter()
        .map(|s| {
            Ok((
                *s,
                KeyPackage::deserialize(&read(&format!("trustee-{s}.key"))?)?,
            ))
        })
        .collect::<anyhow::Result<_>>()?;

    let mut signatures = Vec::new();
    for item in &unsigned.spends {
        let request = SpendAuthRequest {
            sighash: unsigned.sighash,
            alpha: item.alpha,
        };
        // Round 1: each participating trustee commits; round 2: each signs.
        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for (slot, kp) in &trustees {
            let (n, c) = signing::commit(kp, &mut OsRng);
            nonces.insert(dkg::identifier(*slot)?, n);
            commitments.insert(dkg::identifier(*slot)?, c);
        }
        let package = signing::signing_package(&request, commitments);
        let mut shares = BTreeMap::new();
        for (slot, kp) in &trustees {
            let id = dkg::identifier(*slot)?;
            shares.insert(id, signing::sign(&request, &package, &nonces[&id], kp)?);
        }
        signatures.push((
            item.action_index,
            signing::aggregate(&request, &package, &shares, &pubkeys)?,
        ));
    }
    println!(
        "trustees {slots:?} produced {} FROST signature(s)",
        signatures.len()
    );

    let pczt = apply_signatures(unsigned.pczt, unsigned.sighash, &signatures)?;
    let txid = escrow.finalize(pczt)?;
    escrow.broadcast(&mut client, txid).await?;
    println!("broadcast escrow spend: {txid}");
    Ok(())
}
