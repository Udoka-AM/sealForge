//! `sealforge`: organiser and trustee operations. See docs/03-APP-FLOW.md §3.8.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sealforge_cli::{
    keystore::{write_private, Keystore},
    organiser,
    trustee::{self, DkgProgress},
};

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

    /// Where keys are stored (default: $SEALFORGE_HOME or ~/.sealforge).
    #[arg(long, global = true)]
    home: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate an organiser key and register it with the coordinator.
    Init {
        #[arg(long)]
        invite: String,
        #[arg(long)]
        name: String,
    },
    /// Auction management.
    #[command(subcommand)]
    Auction(AuctionCommand),
    /// Trustee operations.
    #[command(subcommand)]
    Trustee(TrusteeCommand),
    /// Escrow tooling for the FROST spike (task 1.5).
    #[command(subcommand)]
    Escrow(EscrowCommand),
}

#[derive(Debug, Subcommand)]
enum AuctionCommand {
    /// Create a draft auction from a TOML config (see examples/auction.toml).
    New { config: PathBuf },
    /// List your auctions.
    List,
    /// Show one of your auctions.
    Show { code: String },
    /// Open a ready auction for bidding.
    Open { code: String },
    /// Cancel an auction (only before bids arrive).
    Cancel { code: String },
    /// Publish a closed auction's result and viewing key so anyone can audit it.
    Publish { code: String },
}

#[derive(Debug, Subcommand)]
enum TrusteeCommand {
    /// Join an auction as a trustee using a one-time invite code.
    Join { invite: String },
    /// Run your side of the key generation. Safe to re-run.
    Dkg {
        code: String,
        /// Keep going until key generation finishes (polls every 10 s).
        #[arg(long)]
        wait: bool,
    },
    /// Review and sign a pending refund or payout batch (task 3.9).
    Sign { batch_id: String },
}

#[derive(Debug, Subcommand)]
enum EscrowCommand {
    /// Run a 2-of-3 DKG in this process and write all three key shares to `out`.
    ///
    /// Testnet/regtest only: one machine holding every share defeats the
    /// threshold. Real auctions use `sealforge trustee dkg` on each trustee's machine.
    LocalDkg {
        #[arg(long)]
        out: PathBuf,
        /// testnet | regtest
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let store = Keystore::new(cli.home.clone().unwrap_or_else(Keystore::default_root));
    let url = cli.coordinator.as_str();

    match cli.command {
        Command::Init { invite, name } => {
            let me = organiser::init(&store, url, &invite, &name).await?;
            println!(
                "Registered organiser \"{}\". Key saved in {}",
                me.display_name,
                store.root().display()
            );
        }
        Command::Auction(cmd) => auction(&store, url, cmd).await?,
        Command::Trustee(TrusteeCommand::Join { invite }) => {
            let joined = trustee::join(&store, url, &invite).await?;
            println!(
                "Joined auction {} as trustee {} ({}). Next: sealforge trustee dkg {} --wait",
                joined.auction_code, joined.slot, joined.label, joined.auction_code
            );
        }
        Command::Trustee(TrusteeCommand::Dkg { code, wait }) => loop {
            let progress = trustee::dkg_step(&store, url, &code).await?;
            println!("{}", describe(&progress));
            let finished = matches!(
                progress,
                DkgProgress::Ready { .. }
                    | DkgProgress::Completed {
                        escrow_ua: Some(_),
                        ..
                    }
            );
            let moved = matches!(progress, DkgProgress::SentRound1 | DkgProgress::SentRound2);
            if finished || !wait {
                break;
            }
            if !moved {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        },
        Command::Trustee(other @ TrusteeCommand::Sign { .. }) => {
            anyhow::bail!("`{other:?}` is not implemented yet (task 3.9)")
        }
        Command::Escrow(EscrowCommand::LocalDkg { out, network }) => local_dkg(&out, &network)?,
    }
    Ok(())
}

async fn auction(store: &Keystore, url: &str, cmd: AuctionCommand) -> anyhow::Result<()> {
    match cmd {
        AuctionCommand::New { config } => {
            let created = organiser::auction_new(store, url, &config).await?;
            println!("Created draft auction {}", created.auction.code);
            println!(
                "Send each trustee their one-time invite (out of band, never in a public channel):"
            );
            for invite in &created.trustee_invites {
                println!(
                    "  slot {} ({}): sealforge trustee join {}",
                    invite.slot, invite.label, invite.invite_code
                );
            }
        }
        AuctionCommand::List => print_json(&organiser::auction_list(store, url).await?),
        AuctionCommand::Show { code } => {
            print_json(&organiser::auction_show(store, url, &code).await?)
        }
        AuctionCommand::Open { code } => {
            print_json(&organiser::auction_open(store, url, &code).await?)
        }
        AuctionCommand::Cancel { code } => {
            print_json(&organiser::auction_cancel(store, url, &code).await?)
        }
        AuctionCommand::Publish { code } => {
            let view = organiser::auction_publish(store, url, &code).await?;
            println!(
                "Published {}. Anyone can now audit it: {url}/v1/auctions/{}/result",
                view.code, view.code
            );
        }
    }
    Ok(())
}

fn print_json(value: &impl serde::Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn describe(progress: &DkgProgress) -> String {
    match progress {
        DkgProgress::WaitingForTrustees { joined, total } => {
            format!("Waiting for trustees to join ({joined}/{total})")
        }
        DkgProgress::SentRound1 => "Sent round 1".into(),
        DkgProgress::WaitingForRound1 { received, total } => {
            format!("Waiting for round 1 packages ({received}/{total})")
        }
        DkgProgress::SentRound2 => "Sent round 2".into(),
        DkgProgress::WaitingForRound2 { received, needed } => {
            format!("Waiting for round 2 packages ({received}/{needed})")
        }
        DkgProgress::Completed {
            fingerprint,
            escrow_ua,
        } => format!(
            "Key share saved. Group key fingerprint: {fingerprint}\n\
             Compare this fingerprint with the other trustees over a separate channel.\n{}",
            match escrow_ua {
                Some(ua) => format!("Escrow address: {ua}"),
                None => "Waiting for the other trustees to finish.".into(),
            }
        ),
        DkgProgress::Ready { escrow_ua } => {
            format!("Key generation complete. Escrow address: {escrow_ua}")
        }
    }
}

fn local_dkg(out: &std::path::Path, network: &str) -> anyhow::Result<()> {
    use rand::rngs::OsRng;
    use sealforge_escrow::{address, dkg, Threshold};
    use zcash_protocol::consensus::TEST_NETWORK;

    anyhow::ensure!(
        matches!(network, "testnet" | "regtest"),
        "local-dkg is for testnet/regtest only; mainnet escrows need a real multi-machine DKG"
    );

    let (keys, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng)?;
    for slot in 1..=Threshold::default().max_signers {
        let key_package = &keys[&dkg::identifier(slot)?];
        write_private(
            &out.join(format!("trustee-{slot}.key")),
            hex::encode(key_package.serialize()?).as_bytes(),
        )?;
    }

    let ak = dkg::group_key_bytes(&pubkeys)?;
    let fvk = address::escrow_fvk(&ak, &mut OsRng)?;
    // Regtest shares the testnet address encoding.
    let ua = address::escrow_address(&fvk, &TEST_NETWORK)?;
    let (ufvk, uivk) = address::escrow_view_keys(&fvk, &TEST_NETWORK)?;
    write_private(
        &out.join("escrow.fvk"),
        hex::encode(fvk.to_bytes()).as_bytes(),
    )?;
    write_private(
        &out.join("public-key-package.hex"),
        hex::encode(pubkeys.serialize()?).as_bytes(),
    )?;
    write_private(
        &out.join("escrow.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "network": network,
            "threshold": "2-of-3",
            "group_key_ak": hex::encode(ak),
            "escrow_ua": ua,
            "ufvk": ufvk,
            "uivk": uivk,
        }))?
        .as_bytes(),
    )?;

    println!("Escrow UA ({network}): {ua}");
    println!(
        "Wrote 3 trustee key shares, escrow.fvk and escrow.json to {}",
        out.display()
    );
    println!("Keep these files private. They control the escrow.");
    Ok(())
}
