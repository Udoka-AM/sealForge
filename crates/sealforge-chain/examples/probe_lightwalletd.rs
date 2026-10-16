//! Check a lightwallet server before pointing sealForge at it:
//! `cargo run -p sealforge-chain --example probe_lightwalletd -- https://zec.rocks:443`
//! Prints the chain, tip and the network upgrades the server reports (Ironwood = NU6.3).

use sealforge_chain::connect;
use zcash_client_backend::proto::service::Empty;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:8137".into());
    let mut client = connect(&url).await?;
    let info = client.get_lightd_info(Empty {}).await?.into_inner();
    println!("server:        {url}");
    println!("vendor:        {} {}", info.vendor, info.version);
    println!("chain:         {}", info.chain_name);
    println!("tip:           {}", info.block_height);
    println!("branch id:     {}", info.consensus_branch_id);
    println!("{info:#?}");
    Ok(())
}
