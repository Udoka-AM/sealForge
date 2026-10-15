use clap::Parser;
use sealforge_api::types::{Network, PublishedResult};
use sealforge_auditor::audit;
use sealforge_chain::{audit::scan_incoming, connect, ChainParams};
use sealforge_core::{memo::BidMemo, selection::Payment, Zatoshi};

/// Recompute a published sealForge auction result from the chain, using only
/// the published viewing key and a lightwallet server you choose.
#[derive(Debug, Parser)]
#[command(name = "sealforge-audit", version)]
struct Args {
    /// The published result: a file, or its URL
    /// (e.g. https://…/v1/auctions/<code>/result).
    #[arg(long)]
    published: String,
    /// Escrow incoming viewing key. Defaults to the one in the published result;
    /// pass it to check against a key you got elsewhere.
    #[arg(long)]
    ivk: Option<String>,
    /// First block to scan. Defaults to the published scan range.
    #[arg(long)]
    from: Option<u32>,
    /// Last block to scan (the close height). Defaults to the published range.
    #[arg(long)]
    to: Option<u32>,
    /// Lightwallet (Zaino / lightwalletd) gRPC endpoint.
    #[arg(long, env = "SF_ZAINO_URL")]
    endpoint: String,
    /// Print the report as JSON.
    #[arg(long)]
    json: bool,
}

async fn load(published: &str) -> anyhow::Result<PublishedResult> {
    let text = if published.starts_with("http://") || published.starts_with("https://") {
        reqwest::get(published)
            .await?
            .error_for_status()?
            .text()
            .await?
    } else {
        std::fs::read_to_string(published)?
    };
    Ok(serde_json::from_str(&text)?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let published = load(&args.published).await?;
    let params = match published.network {
        Network::Mainnet => ChainParams::Mainnet,
        Network::Testnet => ChainParams::Testnet,
        Network::Regtest => ChainParams::regtest(),
    };
    let ivk = args.ivk.as_deref().unwrap_or(&published.uivk);
    let from = args.from.unwrap_or(published.scan_from_height);
    let to = args.to.unwrap_or(published.scan_to_height);

    let started = std::time::Instant::now();
    let mut client = connect(&args.endpoint).await?;
    let found = scan_incoming(&mut client, params, ivk, from, to).await?;
    let scan_secs = started.elapsed().as_secs_f64();
    let payments: Vec<Payment> = found
        .iter()
        .map(|p| Payment {
            txid: p.txid,
            output_index: p.output_index,
            block_height: p.block_height,
            amount: Zatoshi(p.amount_zat),
            memo: p.memo.as_deref().and_then(|m| BidMemo::decode(m).ok()),
        })
        .collect();
    let report = audit(&published, &payments);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Scanned blocks {from}–{to} of {} in {scan_secs:.1} s: {} payment(s) to the escrow.\n",
            published.auction_code,
            found.len()
        );
        for f in &report.fields {
            println!(
                "{}  {:<24} published {:<66} recomputed {}",
                if f.ok { "✓" } else { "✗" },
                f.field,
                f.published,
                f.recomputed
            );
        }
        println!(
            "\n{}",
            if report.matches {
                "Match: the published result is exactly what the chain says."
            } else {
                "MISMATCH: the published result does not follow from the chain."
            }
        );
    }
    std::process::exit(if report.matches { 0 } else { 1 });
}
