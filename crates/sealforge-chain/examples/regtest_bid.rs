//! End to end on the local regtest chain: a dev wallet places a real shielded
//! bid (with an `SF1:` memo) into a FROST escrow, and the escrow wallet finds it.
//!
//!   regtest_bid address <dev.sqlite> <birthday>      print the dev wallet's UA (mine to it)
//!   regtest_bid bid <dev.sqlite> <escrow-ua> <escrow-ufvk> <escrow.sqlite> <auction-code> <zat> [salt-byte]
//!   (prints the bid receipt JSON, as the web app would save it)

use sealforge_chain::{connect, dev::DevWallet, ChainParams, EscrowWallet};
use sealforge_core::{memo::BidMemo, receipt::commitment};

const SEED: [u8; 32] = [42; 32]; // regtest-only test seed
const ZAINO: &str = "http://127.0.0.1:8137";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let params = ChainParams::regtest();
    let mut client = connect(ZAINO).await?;
    match args.get(1).map(String::as_str) {
        Some("address") => {
            let birthday: u32 = args[3].parse()?;
            let wallet =
                DevWallet::open_or_create(args[2].as_ref(), params, SEED, birthday, &mut client)
                    .await?;
            println!("{}", wallet.address()?);
        }
        Some("bid") => {
            let (dev_path, escrow_ua, escrow_ufvk, escrow_path, code) =
                (&args[2], &args[3], &args[4], &args[5], &args[6]);
            let amount: u64 = args[7].parse()?;
            let mut dev =
                DevWallet::open_or_create(dev_path.as_ref(), params, SEED, 1, &mut client).await?;
            dev.sync(&mut client).await?;
            println!("dev wallet spendable: {} zat", dev.spendable_zat()?);

            // A real bid memo: commitment over the code, refund address and a salt.
            let refund_ua = dev.address()?;
            let salt = [args.get(8).map(|b| b.parse()).transpose()?.unwrap_or(7u8); 32];
            let memo = BidMemo {
                auction_code: code.clone(),
                commitment: commitment(code, &refund_ua, &salt),
                refund_ua,
            }
            .encode()?;
            let txid = dev
                .send(&mut client, escrow_ua, amount, Some(&memo))
                .await?;
            println!("bid sent: {txid}");
            println!("MINE NOW");
            // The caller mines a block, then re-runs `scan`.
            let _ = (escrow_ufvk, escrow_path);
        }
        Some("scan") => {
            let (escrow_ufvk, escrow_path) = (&args[2], std::path::PathBuf::from(&args[3]));
            let mut escrow = if escrow_path.exists() {
                EscrowWallet::open(&escrow_path, params)?
            } else {
                EscrowWallet::create(&escrow_path, params, escrow_ufvk, 1, &mut client).await?
            };
            escrow.sync(&mut client).await?;
            for p in escrow.received()?.iter().filter(|p| p.memo.is_some()) {
                let parsed = BidMemo::decode(p.memo.as_deref().unwrap_or_default());
                println!(
                    "bid at height {}: {} zat, memo parses: {}, code {:?}",
                    p.block_height,
                    p.amount_zat,
                    parsed.is_ok(),
                    parsed.as_ref().map(|m| m.auction_code.clone()).ok()
                );
            }
        }
        _ => anyhow::bail!("usage: regtest_bid address|bid|scan ..."),
    }
    Ok(())
}
