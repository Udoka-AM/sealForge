//! `sealforge`: organiser and trustee operations. See docs/03-APP-FLOW.md §3.8.

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
    anyhow::bail!(
        "`{:?}` is not implemented yet (see docs/06-IMPLEMENTATION-PLAN.md)",
        cli.command
    )
}
