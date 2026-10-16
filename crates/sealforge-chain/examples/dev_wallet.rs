//! A small test wallet for any network (regtest, testnet), for running demo
//! auctions without a phone wallet. Never use it with real funds.
//!
//!   dev_wallet <network> <wallet.sqlite> <seed-file> address
//!   dev_wallet <network> <wallet.sqlite> <seed-file> balance
//!   dev_wallet <network> <wallet.sqlite> <seed-file> send <to-ua> <zat> <memo>
//!
//! The seed file (32 random bytes, hex, 0600) is created on first use.
//! Endpoint: SF_LIGHTWALLETD (default: testnet.zec.rocks for testnet, local Zaino for regtest).

use std::{io::Write, path::Path};

use anyhow::{anyhow, bail, Context};
use sealforge_chain::{chain_tip, connect, dev::DevWallet, ChainParams};

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
        other => bail!("unknown command {other:?}"),
    }
    Ok(())
}
