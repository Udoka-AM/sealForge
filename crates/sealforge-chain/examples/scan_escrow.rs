//! Sync an escrow wallet against a lightwalletd/Zaino server and list what it received.
//!
//! cargo run -p sealforge-chain --example scan_escrow -- <ufvk> <wallet.sqlite> [server] [birthday]

use sealforge_chain::{connect, ChainParams, EscrowWallet};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let ufvk = args.get(1).expect("ufvk");
    let path = std::path::PathBuf::from(args.get(2).expect("wallet path"));
    let server = args
        .get(3)
        .map(String::as_str)
        .unwrap_or("http://127.0.0.1:8137");
    let birthday: u32 = args.get(4).map(|b| b.parse().unwrap()).unwrap_or(1);
    let params = ChainParams::regtest();

    let mut client = connect(server).await?;
    let mut wallet = if path.exists() {
        EscrowWallet::open(&path, params)?
    } else {
        EscrowWallet::create(&path, params, ufvk, birthday, &mut client).await?
    };
    let started = std::time::Instant::now();
    wallet.sync(&mut client).await?;
    let received = wallet.received()?;
    println!(
        "scanned to {:?} in {:.1}s: {} payments, {} zat total",
        wallet.scanned_height()?,
        started.elapsed().as_secs_f64(),
        received.len(),
        received.iter().map(|p| p.amount_zat).sum::<u64>()
    );
    for p in received.iter().take(3).chain(received.iter().rev().take(2)) {
        println!(
            "  height {} amount {} memo {:?}",
            p.block_height, p.amount_zat, p.memo
        );
    }
    Ok(())
}
