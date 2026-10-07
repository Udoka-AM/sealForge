use clap::Parser;

/// Recompute a published sealForge auction result from the chain.
#[derive(Debug, Parser)]
#[command(name = "sealforge-audit", version)]
struct Args {
    /// Published incoming viewing key of the escrow.
    #[arg(long)]
    ivk: String,
    /// First block height to scan.
    #[arg(long)]
    from: u32,
    /// Last block height to scan (the auction's close height).
    #[arg(long)]
    to: u32,
    /// Path to the published auction JSON (rules, root).
    #[arg(long)]
    published: std::path::PathBuf,
    /// Zaino / lightwalletd gRPC endpoint.
    #[arg(
        long,
        env = "SF_ZAINO_URL",
        default_value = "https://zaino.testnet.example:443"
    )]
    endpoint: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let _ = (
        &args.ivk,
        args.from,
        args.to,
        &args.published,
        &args.endpoint,
    );
    anyhow::bail!("not implemented yet: Milestone 3, task 3.4")
}
