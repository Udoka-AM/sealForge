//! Drive the browser wallet's code natively against a real chain, to check
//! sync, witnesses and payments end to end before the page uses them.
//!
//!   wallet_testnet <dir> init                  new phrase (0600) and wallet born at the tip
//!   wallet_testnet <dir> sync                  sync to the tip, print the balance and notes
//!   wallet_testnet <dir> pay <ua> <zat> [memo] pay, broadcast, and wait for the confirmation
//!
//! Network: SF_NETWORK (default testnet). Endpoint: SF_LIGHTWALLETD.

use std::{io::Write, path::Path};

use anyhow::{anyhow, bail, Context};
use sealforge_chain::{chain_tip, connect, Client};
use sealforge_wallet::{
    chain_params, phrase_from_entropy, Keys, PaymentRequest, Wallet, WalletState,
};
use zcash_client_backend::proto::service::{BlockId, BlockRange, RawTransaction};

fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    Ok(opts.open(path)?.write_all(bytes)?)
}

async fn sync(wallet: &mut Wallet, client: &mut Client) -> anyhow::Result<()> {
    let tip = chain_tip(client).await?;
    let from = wallet.scanned_height() + 1;
    if from > tip {
        return Ok(());
    }
    let mut stream = client
        .get_block_range(BlockRange {
            start: Some(BlockId {
                height: from as u64,
                hash: vec![],
            }),
            end: Some(BlockId {
                height: tip as u64,
                hash: vec![],
            }),
            ..Default::default()
        })
        .await?
        .into_inner();
    while let Some(block) = stream.message().await? {
        match wallet.scan_block(&block) {
            Ok(out) => {
                for n in &out.received {
                    println!(
                        "  block {}: received {} zat ({}{})",
                        out.height,
                        n.value_zat,
                        if n.change { "change" } else { "address " },
                        n.address_index.map(|i| i.to_string()).unwrap_or_default()
                    );
                }
                for t in &out.confirmed {
                    println!("  block {}: our payment {t} confirmed", out.height);
                }
            }
            Err(sealforge_wallet::ScanError::Reorg) => {
                println!("  reorg at {}: rescanning from the birthday", block.height);
                wallet.reset().map_err(|e| anyhow!(e))?;
                return Box::pin(sync(wallet, client)).await;
            }
            Err(e) => bail!("scan failed: {e}"),
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, dir, cmd, rest @ ..] = args.as_slice() else {
        bail!("usage: wallet_testnet <dir> init|sync|pay …");
    };
    let network = std::env::var("SF_NETWORK").unwrap_or_else(|_| "testnet".into());
    let params = chain_params(&network).map_err(|e| anyhow!(e))?;
    let url =
        std::env::var("SF_LIGHTWALLETD").unwrap_or_else(|_| "https://testnet.zec.rocks:443".into());
    let mut client = connect(&url)
        .await
        .with_context(|| format!("connecting to {url}"))?;
    let dir = Path::new(dir);
    let (phrase_path, state_path) = (dir.join("phrase"), dir.join("state.json"));

    if cmd == "init" {
        std::fs::create_dir_all(dir)?;
        if phrase_path.exists() {
            bail!("{} exists", phrase_path.display());
        }
        let mut entropy = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut entropy);
        let phrase = phrase_from_entropy(entropy).map_err(|e| anyhow!(e))?;
        write_private(&phrase_path, phrase.as_bytes())?;
        let birthday = chain_tip(&mut client).await?;
        let tree = client
            .get_tree_state(BlockId {
                height: (birthday - 1) as u64,
                hash: vec![],
            })
            .await?
            .into_inner();
        let keys = Keys::from_phrase(params, &phrase).map_err(|e| anyhow!(e))?;
        let wallet = Wallet::new(keys, &network, birthday, &tree).map_err(|e| anyhow!(e))?;
        write_private(&state_path, &serde_json::to_vec(&wallet.to_state())?)?;
        println!("born at {birthday}; fund {}", wallet.funding_address());
        return Ok(());
    }

    let phrase = std::fs::read_to_string(&phrase_path)?;
    let state: WalletState = serde_json::from_slice(&std::fs::read(&state_path)?)?;
    let keys = Keys::from_phrase(params, phrase.trim()).map_err(|e| anyhow!(e))?;
    let mut wallet = Wallet::from_state(keys, &state).map_err(|e| anyhow!(e))?;
    sync(&mut wallet, &mut client).await?;
    write_private(&state_path, &serde_json::to_vec(&wallet.to_state())?)?;
    println!(
        "scanned to {}: {:?}",
        wallet.scanned_height(),
        wallet.balance()
    );

    match cmd.as_str() {
        "sync" => {
            for n in wallet.notes() {
                println!(
                    "  {:?} {} zat at {} {}",
                    n.pool,
                    n.value_zat,
                    n.height,
                    if n.spent_txid.is_some() {
                        "(spent)"
                    } else {
                        ""
                    }
                );
            }
        }
        "pay" => {
            let [to, zat, memo @ ..] = rest else {
                bail!("pay <ua> <zat> [memo]");
            };
            let started = std::time::Instant::now();
            let built = wallet
                .pay(&PaymentRequest {
                    to: to.clone(),
                    amount_zat: zat.parse()?,
                    memo: memo.first().cloned(),
                    label: Some("cli".into()),
                })
                .map_err(|e| anyhow!(e))?;
            println!(
                "built {} (fee {} zat) in {:.1?}",
                built.txid,
                built.fee_zat,
                started.elapsed()
            );
            write_private(&state_path, &serde_json::to_vec(&wallet.to_state())?)?;
            let sent = client
                .send_transaction(RawTransaction {
                    data: built.raw,
                    height: 0,
                })
                .await?
                .into_inner();
            if sent.error_code != 0 {
                wallet.forget_payment(&built.txid);
                write_private(&state_path, &serde_json::to_vec(&wallet.to_state())?)?;
                bail!("refused: {} {}", sent.error_code, sent.error_message);
            }
            println!(
                "broadcast; waiting for it to confirm (balance {:?})",
                wallet.balance()
            );
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                sync(&mut wallet, &mut client).await?;
                write_private(&state_path, &serde_json::to_vec(&wallet.to_state())?)?;
                let tx = wallet
                    .sent()
                    .iter()
                    .find(|t| t.txid == built.txid)
                    .expect("recorded");
                if let Some(h) = tx.mined_height {
                    println!("confirmed at {h}: {:?}", wallet.balance());
                    break;
                }
                if tx.expired {
                    bail!("expired without confirming");
                }
            }
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}
