//! A small test wallet for any network (regtest, testnet), for running demo
//! auctions without a phone wallet. Never use it with real funds.
//!
//!   dev_wallet <network> <wallet.sqlite> <seed-file> address
//!   dev_wallet <network> <wallet.sqlite> <seed-file> balance
//!   dev_wallet <network> <wallet.sqlite> <seed-file> send <to-ua> <zat> <memo>
//!   dev_wallet <network> <wallet.sqlite> <seed-file> fan-out <zat-each> <to-ua>…
//!   dev_wallet <network> <wallet.sqlite> <seed-file> bid <receipt.json> <code> <escrow-ua> <deposit-zat> [price-zat]
//!   dev_wallet <network> <wallet.sqlite> <seed-file> reveal <receipt.json> <escrow-ua>
//!
//! `bid` builds the same memo and receipt as the browser bid flow, with this
//! wallet's address as the refund address (one wallet per simulated bidder).
//! With a price it is a sealed bid: the deposit memo commits to the price, and
//! `reveal` sends the `SF1R:` memo after bidding closes.
//!
//! The seed file (32 random bytes, hex, 0600) is created on first use.
//! Endpoint: SF_LIGHTWALLETD (default: testnet.zec.rocks for testnet, local Zaino for regtest).

use std::{io::Write, path::Path};

use anyhow::{anyhow, bail, Context};
use sealforge_chain::{chain_tip, connect, dev::DevWallet, ChainParams};
use sealforge_core::{
    memo::{BidMemo, RevealMemo},
    receipt::{commitment, price_commitment, Receipt},
};

/// What a reveal pays the escrow (refunded with the deposit when it counts).
const REVEAL_ZAT: u64 = 10_000;

fn seed(path: &Path) -> anyhow::Result<[u8; 32]> {
    if !path.exists() {
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        opts.open(path)?.write_all(hex::encode(seed).as_bytes())?;
        return Ok(seed);
    }
    hex::decode(std::fs::read_to_string(path)?.trim())?
        .try_into()
        .map_err(|_| anyhow!("bad seed file"))
}

fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    Ok(opts.open(path)?.write_all(bytes)?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, network, db, seed_file, cmd, rest @ ..] = args.as_slice() else {
        bail!("usage: dev_wallet <regtest|testnet> <wallet.sqlite> <seed-file> address|balance|send …");
    };
    let (params, default_url) = match network.as_str() {
        "testnet" => (ChainParams::Testnet, "https://testnet.zec.rocks:443"),
        "regtest" => (ChainParams::regtest(), "http://127.0.0.1:8137"),
        other => bail!("dev_wallet is for test networks only, not {other:?}"),
    };
    let url = std::env::var("SF_LIGHTWALLETD").unwrap_or_else(|_| default_url.into());
    let mut client = connect(&url)
        .await
        .with_context(|| format!("connecting to {url}"))?;
    let birthday = chain_tip(&mut client).await?;
    let seed = seed(Path::new(seed_file))?;
    let mut wallet =
        DevWallet::open_or_create(Path::new(db), params, seed, birthday, &mut client).await?;
    match cmd.as_str() {
        "address" => println!("{}", wallet.address()?),
        "balance" => {
            wallet.sync(&mut client).await?;
            println!("{} zat spendable", wallet.spendable_zat()?);
        }
        "send" => {
            let [to, zat, memo] = rest else {
                bail!("send <to-ua> <zat> <memo>");
            };
            wallet.sync(&mut client).await?;
            let txid = wallet
                .send(&mut client, to, zat.parse()?, Some(memo.as_str()))
                .await?;
            println!("sent: {txid}");
        }
        "fan-out" => {
            let [zat, to @ ..] = rest else {
                bail!("fan-out <zat-each> <to-ua>…");
            };
            let zat: u64 = zat.parse()?;
            let outputs: Vec<_> = to.iter().map(|t| (t.as_str(), zat, None)).collect();
            wallet.sync(&mut client).await?;
            println!("sent: {}", wallet.send_many(&mut client, &outputs).await?);
        }
        "bid" => {
            let (receipt_path, code, escrow, deposit, price) = match rest {
                [r, c, e, d] => (r, c, e, d.parse::<u64>()?, None),
                [r, c, e, d, p] => (r, c, e, d.parse::<u64>()?, Some(p.parse::<u64>()?)),
                _ => bail!("bid <receipt.json> <code> <escrow-ua> <deposit-zat> [price-zat]"),
            };
            if Path::new(receipt_path).exists() {
                bail!("{receipt_path} exists: this bidder already bid");
            }
            let refund_ua = wallet.address()?;
            let mut salt = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut salt);
            let commitment = match price {
                Some(p) => price_commitment(code, &refund_ua, p, &salt),
                None => commitment(code, &refund_ua, &salt),
            };
            let receipt = Receipt {
                version: if price.is_some() { 2 } else { 1 },
                auction_code: code.clone(),
                amount_zat: deposit,
                refund_ua: refund_ua.clone(),
                salt,
                commitment,
                price_zat: price,
            };
            let memo = BidMemo {
                auction_code: code.clone(),
                commitment,
                refund_ua,
            }
            .encode()?;
            wallet.sync(&mut client).await?;
            let txid = wallet
                .send(&mut client, escrow, deposit, Some(&memo))
                .await?;
            write_private(
                Path::new(receipt_path),
                &serde_json::to_vec_pretty(&receipt)?,
            )?;
            println!("bid sent: {txid}");
        }
        "reveal" => {
            let [receipt_path, escrow] = rest else {
                bail!("reveal <receipt.json> <escrow-ua>");
            };
            let receipt: Receipt = serde_json::from_slice(&std::fs::read(receipt_path)?)?;
            let price_zat = receipt
                .price_zat
                .ok_or_else(|| anyhow!("not a sealed bid: nothing to reveal"))?;
            let memo = RevealMemo {
                auction_code: receipt.auction_code,
                commitment: receipt.commitment,
                price_zat,
                salt: receipt.salt,
            }
            .encode()?;
            wallet.sync(&mut client).await?;
            let txid = wallet
                .send(&mut client, escrow, REVEAL_ZAT, Some(&memo))
                .await?;
            println!("reveal sent: {txid}");
        }
        other => bail!("unknown command {other:?}"),
    }
    Ok(())
}
