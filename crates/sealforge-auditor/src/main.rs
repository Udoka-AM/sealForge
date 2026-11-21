use clap::Parser;
use sealforge_api::types::{Network, PublishedResult};
use sealforge_auditor::{
    audit_chain, check_escrow_result, require_identities, ChainPayment, EscrowVerdict,
    IdentityVerdict, TrusteeVerdict,
};
use sealforge_chain::{audit::scan_incoming, connect, ChainParams};

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
    /// A trustee's key for this auction, as the trustee gave it to you:
    /// `SLOT=HEX`. Repeat for each trustee. A published key that differs fails
    /// the audit, which catches a coordinator that invented its own trustees.
    #[arg(long = "trustee-key", value_parser = parse_pin)]
    trustee_keys: Vec<(u16, String)>,
    /// A trustee's long-term key (hex), as the trustee publishes it on its own
    /// site or account. Repeat for each trustee you know. The audit fails
    /// unless each one certifies a trustee of this auction.
    #[arg(long = "trustee-identity")]
    trustee_identities: Vec<String>,
    /// Don't fail when no trustee attestations are published (auctions from
    /// before attestations existed).
    #[arg(long)]
    allow_unattested: bool,
    /// Print the report as JSON.
    #[arg(long)]
    json: bool,
}

fn parse_pin(value: &str) -> Result<(u16, String), String> {
    let (slot, key) = value
        .split_once('=')
        .ok_or("expected SLOT=HEX, e.g. 1=3b6a…")?;
    let slot = slot.parse().map_err(|_| format!("bad slot {slot}"))?;
    match hex::decode(key) {
        Ok(bytes) if bytes.len() == 32 => Ok((slot, key.to_ascii_lowercase())),
        _ => Err("the key must be 32 bytes of hex".into()),
    }
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
    let payments: Vec<ChainPayment> = found
        .iter()
        .map(|p| ChainPayment {
            txid: p.txid,
            output_index: p.output_index,
            block_height: p.block_height,
            amount_zat: p.amount_zat,
            memo: p.memo.clone(),
        })
        .collect();
    let report = audit_chain(&published, &payments);

    // The escrow: did every trustee sign the address bidders paid, and does
    // the viewing key we just scanned with belong to that address?
    let mut escrow = check_escrow_result(&published, &args.trustee_keys);
    require_identities(&mut escrow, &args.trustee_identities);
    let key_address = sealforge_scan::escrow_address(&params, ivk);
    let key_matches = match (&key_address, &published.escrow_ua) {
        (Ok(derived), Some(ua)) => Some(derived == ua),
        _ => None,
    };
    let escrow_ok = key_matches != Some(false)
        && match escrow.verdict {
            EscrowVerdict::Verified => key_matches == Some(true),
            EscrowVerdict::Unattested => args.allow_unattested,
            EscrowVerdict::Failed => false,
        };

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "result": report,
                "escrow": escrow,
                "viewing_key_address": key_address.as_ref().ok(),
                "viewing_key_matches_escrow": key_matches,
                "escrow_ok": escrow_ok,
            }))?
        );
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

        println!("\nEscrow");
        match (&key_address, key_matches) {
            (_, Some(true)) => {
                println!("✓  the viewing key receives at the published escrow address")
            }
            (Ok(derived), Some(false)) => println!(
                "✗  the viewing key receives at {derived}, not the published escrow address"
            ),
            (Err(e), _) => println!("✗  could not read the viewing key: {e}"),
            (Ok(_), None) => println!("–  no escrow address published to compare with"),
        }
        for t in &escrow.trustees {
            let key = t.pubkey.as_deref().unwrap_or("(none)");
            let what = match t.verdict {
                TrusteeVerdict::Signed => "✓  signed",
                TrusteeVerdict::Missing => "✗  no attestation",
                TrusteeVerdict::Invalid => "✗  INVALID signature",
                TrusteeVerdict::WrongKey => "✗  NOT the key you pinned",
            };
            println!("{what:<26} slot {} {:<16} key {key}", t.slot, t.label);
            match t.identity {
                IdentityVerdict::Certified => println!(
                    "   vouched for by long-term key {} (published at {})",
                    t.identity_pubkey.as_deref().unwrap_or_default(),
                    t.identity_published_at.as_deref().unwrap_or_default()
                ),
                IdentityVerdict::None => {
                    println!("   no long-term key: nothing ties this key to a known trustee")
                }
                IdentityVerdict::Invalid => println!("✗  its long-term key certificate is INVALID"),
            }
        }
        for problem in &escrow.problems {
            println!("✗  {problem}");
        }
        if args.trustee_keys.is_empty()
            && args.trustee_identities.is_empty()
            && escrow.verdict == EscrowVerdict::Verified
        {
            println!(
                "   These keys come from the coordinator. To rule out a stand-in trustee, compare each\n   long-term key above with the place the trustee publishes it, or pass\n   --trustee-identity HEX for each long-term key you know."
            );
        }
        println!(
            "\n{}",
            match (escrow_ok, escrow.verdict) {
                (true, EscrowVerdict::Verified) => format!(
                    "Escrow verified: all {} trustees signed the address bidders paid.",
                    escrow.total
                ),
                (true, _) => "Escrow not attested (allowed by --allow-unattested).".to_owned(),
                (false, EscrowVerdict::Unattested) => {
                    "ESCROW UNVERIFIED: no trustee attestations are published. Pass\n--allow-unattested for auctions from before attestations existed."
                        .to_owned()
                }
                (false, _) =>
                    "ESCROW MISMATCH: the escrow address is not the one the trustees control."
                        .to_owned(),
            }
        );
    }
    std::process::exit(if report.matches && escrow_ok { 0 } else { 1 });
}
