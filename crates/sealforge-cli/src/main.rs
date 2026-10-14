//! `sealforge`: organiser and trustee operations. See docs/03-APP-FLOW.md §3.8.

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "sealforge",
    version,
    about = "Run and co-sign sealForge auctions"
)]
struct Cli {
    /// Coordinator base URL.
    #[arg(
        long,
        global = true,
        env = "SF_COORDINATOR_URL",
        default_value = "http://127.0.0.1:8080"
    )]
    coordinator: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate an organiser key and register it with the coordinator (task 2.6).
    Init {
        #[arg(long)]
        invite: String,
        #[arg(long)]
        name: String,
    },
    /// Auction management (task 2.6).
    #[command(subcommand)]
    Auction(AuctionCommand),
    /// Trustee operations (tasks 2.4 and 3.9).
    #[command(subcommand)]
    Trustee(TrusteeCommand),
    /// Escrow tooling for the FROST spike (task 1.5).
    #[command(subcommand)]
    Escrow(EscrowCommand),
}

#[derive(Debug, Subcommand)]
enum EscrowCommand {
    /// Run a 2-of-3 DKG in this process and write all three key shares to `out`.
    ///
    /// Testnet/regtest only: one machine holding every share defeats the
    /// threshold. Real auctions run the DKG across trustees (task 2.4).
    LocalDkg {
        #[arg(long)]
        out: std::path::PathBuf,
        /// testnet | regtest
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}

#[derive(Debug, Subcommand)]
enum AuctionCommand {
    /// Create a draft auction from a TOML config.
    New { config: std::path::PathBuf },
    /// Open a ready auction.
    Open { code: String },
    /// Publish the result, IVK and Merkle root.
    Publish { code: String },
}

#[derive(Debug, Subcommand)]
enum TrusteeCommand {
    /// Join an auction as a trustee using a one-time invite code.
    Join { invite: String },
    /// Run the FROST DKG rounds for an auction.
    Dkg { code: String },
    /// Review and sign a pending refund or payout batch.
    Sign { batch_id: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Escrow(EscrowCommand::LocalDkg { out, network }) => local_dkg(&out, &network),
        other => {
            anyhow::bail!("`{other:?}` is not implemented yet (see docs/06-IMPLEMENTATION-PLAN.md)")
        }
    }
}

/// Create a file readable only by the owner. Never overwrites.
fn write_private(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .with_context(|| format!("creating {} (refusing to overwrite)", path.display()))?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}

fn local_dkg(out: &std::path::Path, network: &str) -> anyhow::Result<()> {
    use rand::rngs::OsRng;
    use sealforge_escrow::{address, dkg, Threshold};
    use zcash_protocol::consensus::TEST_NETWORK;

    anyhow::ensure!(
        matches!(network, "testnet" | "regtest"),
        "local-dkg is for testnet/regtest only; mainnet escrows need a real multi-machine DKG"
    );
    std::fs::create_dir_all(out)?;

    let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng)?;
    for slot in 1..=Threshold::default().max_signers {
        let key_package = &keys[&dkg::identifier(slot)?];
        write_private(
            &out.join(format!("trustee-{slot}.key")),
            &hex::encode(key_package.serialize()?),
        )?;
    }

    let ak = dkg::group_key_bytes(&pubkeys)?;
    let fvk = address::escrow_fvk(&ak, &mut OsRng)?;
    // Regtest shares the testnet address encoding.
    let ua = address::escrow_address(&fvk, &TEST_NETWORK)?;
    let (ufvk, uivk) = address::escrow_view_keys(&fvk, &TEST_NETWORK)?;
    write_private(&out.join("escrow.fvk"), &hex::encode(fvk.to_bytes()))?;
    write_private(
        &out.join("public-key-package.hex"),
        &hex::encode(pubkeys.serialize()?),
    )?;
    write_private(
        &out.join("escrow.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "network": network,
            "threshold": "2-of-3",
            "group_key_ak": hex::encode(ak),
            "escrow_ua": ua,
            "ufvk": ufvk,
            "uivk": uivk,
        }))?,
    )?;

    println!("Escrow UA ({network}): {ua}");
    println!(
        "Wrote 3 trustee key shares, escrow.fvk and escrow.json to {}",
        out.display()
    );
    println!("Keep these files private. They control the escrow.");
    Ok(())
}
