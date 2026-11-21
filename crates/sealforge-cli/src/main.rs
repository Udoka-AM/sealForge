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
    /// Print a payment request that funds the escrow's fee buffer (ZIP-317 fees
    /// for refunds and the payout). Whatever fees don't use comes back with the payout.
    FundFees {
        code: String,
        /// Amount in zatoshi (default 0.001 ZEC, enough for about 20 escrow transactions).
        #[arg(long, default_value_t = 100_000)]
        zat: u64,
    },
}

#[derive(Debug, Subcommand)]
enum TrusteeCommand {
    /// Create your long-term trustee key (once) and say where you publish it.
    /// Post the printed key there: bidders compare it with the one that
    /// vouches for you on each auction page.
    Identity {
        /// Where you publish the key: a URL or handle you control
        /// (your website, GitHub, X).
        #[arg(long)]
        published_at: Option<String>,
    },
    /// Join an auction as a trustee using a one-time invite code. If you have
    /// a long-term key, it vouches for your key in this auction.
    Join { invite: String },
    /// Vouch for your key in an auction with your long-term key (if you
    /// joined before you had one).
    Certify { code: String },
    /// Run your side of the key generation. Safe to re-run.
    Dkg {
        code: String,
        /// Keep going until key generation finishes (polls every 10 s).
        #[arg(long)]
        wait: bool,
    },
    /// Check the auction's public page against the escrow address you signed,
    /// and print your key for bidders. Run it once the auction opens.
    Check { code: String },
    /// Print your backup string for an auction: your key share and the
    /// escrow's viewing key. With a second trustee's backup it spends the
    /// escrow, even without the coordinator. Keep it offline; never share it.
    Backup { code: String },
    /// Check a backup string against this machine's keystore without writing
    /// anything: same key share, and a viewing key that gives the address you
    /// attested. `-` reads the string from stdin.
    CheckBackup { code: String, file: PathBuf },
    /// Review and co-sign pending refund/payout batches. Safe to re-run.
    Sign {
        code: String,
        /// Keep going until nothing is waiting on you (polls every 10 s).
        #[arg(long)]
        wait: bool,
        /// Sign for an auction you never attested, from before escrow
        /// attestations existed. Without it, signing refuses.
        #[arg(long)]
        allow_unattested_escrow: bool,
        /// A lightwallet server you trust (gRPC, e.g. https://zec.rocks:443).
        /// Refund batches are checked against the chain through it; without
        /// it they are refused.
        #[arg(long, env = "SF_LIGHTWALLETD_URL")]
        lightwalletd: Option<String>,
    },
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
    /// Restore drill: check that backup strings from enough trustees rebuild
    /// the escrow and sign together with no coordinator (a random spend
    /// authorization, verified like an on-chain one; no transaction is made).
    /// Prints the escrow address to compare with the auction page.
    RecoverCheck {
        /// Backup files, one per trustee (`sealforge trustee backup`).
        #[arg(required = true, num_args = 2..)]
        backups: Vec<PathBuf>,
    },
    /// Without the coordinator, step 1 of 3 (trustee A): from your backup,
    /// scan the escrow and build a sweep of every note to an address both
    /// trustees agreed on. Writes request.json for the other trustee.
    RecoverStart {
        #[arg(long)]
        backup: PathBuf,
        /// A lightwallet server you trust (gRPC).
        #[arg(long, env = "SF_LIGHTWALLETD_URL")]
        lightwalletd: String,
        /// The agreed destination (a unified address with an Orchard receiver).
        #[arg(long)]
        to: String,
        /// The escrow's birthday height, if the backup doesn't record it.
        #[arg(long)]
        birthday: Option<u32>,
        /// Working directory (default: ~/.sealforge/recovery/<escrow>).
        #[arg(long)]
        work: Option<PathBuf>,
    },
    /// Without the coordinator, step 2 of 3 (trustee B): check the request
    /// from your own backup. With --approve, sign it and write the reply.
    RecoverCosign {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        request: PathBuf,
        /// The destination you agreed to (typed by you, not taken from the request).
        #[arg(long)]
        to: String,
        /// Sign after checking. Without it, only shows the check.
        #[arg(long)]
        approve: bool,
        #[arg(long, default_value = "reply.json")]
        out: PathBuf,
    },
    /// Without the coordinator, step 3 of 3 (trustee A): sign with the other
    /// trustee's reply, check the signatures and broadcast.
    RecoverFinish {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        reply: PathBuf,
        #[arg(long, env = "SF_LIGHTWALLETD_URL")]
        lightwalletd: String,
        #[arg(long)]
        work: Option<PathBuf>,
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
        Command::Trustee(TrusteeCommand::Identity { published_at }) => {
            let identity = trustee::long_term_identity(&store, published_at.as_deref())?;
            let key = identity.public_key_hex()?;
            match identity.published_at.as_deref() {
                Some(place) => println!(
                    "Your long-term trustee key:\n\n  sealForge trustee key: {key}\n\nPost that line at {place}, \
                     and keep it there. Each auction you join shows this key vouching for you, \
                     with that place; bidders compare the two."
                ),
                None => println!(
                    "Your long-term trustee key: {key}\nNow say where you publish it: \
                     sealforge trustee identity --published-at <your URL or handle>"
                ),
            }
        }
        Command::Trustee(TrusteeCommand::Join { invite }) => {
            let joined = trustee::join(&store, url, &invite).await?;
            println!(
                "Joined auction {} as trustee {} ({}).",
                joined.auction_code, joined.slot, joined.label
            );
            let code = &joined.auction_code;
            match store.long_term_identity()? {
                Some(lt) if lt.published_at.is_some() => {
                    let key = trustee::certify(&store, url, code).await?;
                    println!("Your long-term key {key} vouches for you in {code}.");
                }
                _ => println!(
                    "No long-term key vouches for you, so bidders can't tell you from a stand-in. \
                     Run `sealforge trustee identity --published-at <URL>`, publish the key, then \
                     `sealforge trustee certify {code}`."
                ),
            }
            println!("Next: sealforge trustee dkg {code} --wait");
        }
        Command::Trustee(TrusteeCommand::Certify { code }) => {
            let key = trustee::certify(&store, url, &code).await?;
            println!("Your long-term key {key} vouches for you in {code}.");
        }
        Command::Trustee(TrusteeCommand::Dkg { code, wait }) => loop {
            let progress = trustee::dkg_step(&store, url, &code).await?;
            println!("{}", describe(&progress, &code));
            // Finished only once this trustee has checked the escrow address.
            let finished = matches!(
                progress,
                DkgProgress::Ready { .. } | DkgProgress::Verified { .. }
            );
            let moved = matches!(
                progress,
                DkgProgress::SentRound1
                    | DkgProgress::SentRound2
                    | DkgProgress::Completed {
                        escrow_ua: Some(_),
                        ..
                    }
            );
            if finished || !wait {
                break;
            }
            if !moved {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        },
        Command::Trustee(TrusteeCommand::Backup { code }) => {
            let text = trustee::backup(&store, url, &code).await?;
            eprintln!(
                "Your backup for {code}. It holds your key share and the escrow's viewing key: \
                 keep it offline (paper or an encrypted drive), never in chat or email. Check it \
                 with `sealforge trustee check-backup {code} <file>`."
            );
            println!("{text}");
        }
        Command::Trustee(TrusteeCommand::CheckBackup { code, file }) => {
            let text = if file.as_os_str() == "-" {
                std::io::read_to_string(std::io::stdin())?
            } else {
                std::fs::read_to_string(&file)?
            };
            let check = trustee::check_backup(&store, &code, &text)?;
            println!(
                "Backup OK: trustee {} of a {}-of-{} escrow for {code}. It restores your key share \
                 and the escrow address {}{}.",
                check.slot,
                check.threshold.0,
                check.threshold.1,
                check.escrow_ua,
                match check.birthday_height {
                    0 => " (birthday height unknown: a restored wallet scans from activation)"
                        .to_owned(),
                    h => format!(" (scan from height {h})"),
                }
            );
        }
        Command::Trustee(TrusteeCommand::Check { code }) => {
            let key = trustee::my_key(&store, &code)?;
            match trustee::check_public(&store, url, &code).await? {
                trustee::PublicCheck::Matches {
                    escrow_ua,
                    signed,
                    total,
                    ..
                } => println!(
                    "The public page for {code} shows the escrow address you signed ({escrow_ua}), \
                     lists your key, and {signed} of {total} trustee attestations verify."
                ),
                trustee::PublicCheck::NotPublicYet => {
                    println!("{code} isn't public yet. Run this again once the organiser opens it.")
                }
                trustee::PublicCheck::NotAttested => println!(
                    "You haven't checked the escrow for {code}. Run: sealforge trustee dkg {code}"
                ),
            }
            println!(
                "Your trustee key for {code}: {key}\nPost it where bidders can find it (your site or social account) \
                 so they can pin it: the auction page shows each trustee's key."
            );
        }
        Command::Trustee(TrusteeCommand::Sign {
            code,
            wait,
            allow_unattested_escrow,
            lightwalletd,
        }) => loop {
            for batch in trustee::pending_batches(&store, url, &code).await? {
                if batch.status == "awaiting_signatures" && !batch.i_shared {
                    println!("Reviewing {}", trustee::describe_batch(&batch));
                }
            }
            let policy = trustee::SignPolicy {
                allow_unattested_escrow,
                lightwalletd: lightwalletd.clone(),
                ..Default::default()
            };
            let progress = trustee::sign_step_with(&store, url, &code, policy).await?;
            if progress.is_empty() {
                println!("Nothing to sign for {code}.");
            }
            for p in &progress {
                println!("{p}");
            }
            let waiting = progress.iter().any(|p| {
                matches!(
                    p,
                    trustee::SignProgress::Committed { .. }
                        | trustee::SignProgress::WaitingForSecondSigner { .. }
                        | trustee::SignProgress::WaitingForCoSignerCheck { .. }
                )
            });
            if !wait || !waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        },
        Command::Escrow(EscrowCommand::LocalDkg { out, network }) => local_dkg(&out, &network)?,
        Command::Escrow(EscrowCommand::RecoverStart {
            backup,
            lightwalletd,
            to,
            birthday,
            work,
        }) => {
            let work = match work {
                Some(w) => w,
                None => sealforge_cli::recover::default_work_dir(store.root(), &backup)?,
            };
            let started =
                sealforge_cli::recover::start(&backup, &lightwalletd, &to, &work, birthday).await?;
            let r = &started.request;
            println!(
                "Built a sweep of {} note(s), {} zat after the fee, from {} to {}.\n\
                 Send {} to the other trustee (it holds no secrets), and agree the destination \
                 with them by voice. They run:\n  sealforge escrow recover-cosign --backup \
                 <their backup> --request request.json --to {}\nThen, with their reply.json:\n  \
                 sealforge escrow recover-finish --backup {} --reply reply.json --lightwalletd {}",
                started.notes,
                r.amount_zat,
                r.escrow_ua,
                r.to,
                work.join("request.json").display(),
                r.to,
                backup.display(),
                lightwalletd
            );
            if started.more_left {
                println!(
                    "More notes are left than one transaction may spend: run this again once \
                     this sweep is mined."
                );
            }
        }
        Command::Escrow(EscrowCommand::RecoverCosign {
            backup,
            request,
            to,
            approve,
            out,
        }) => {
            let request: sealforge_cli::recover::Request =
                serde_json::from_slice(&std::fs::read(&request)?)?;
            let reviewed = sealforge_cli::recover::cosign(&backup, &request, &to, approve)?;
            println!("Checked the transaction from your own backup:");
            for line in &reviewed.verified.lines {
                println!("  {line}");
            }
            for w in &reviewed.verified.warnings {
                println!("  warning: {w}");
            }
            println!(
                "  sighash {}, expires at height {}",
                hex::encode(reviewed.verified.sighash),
                reviewed.verified.expiry_height
            );
            match reviewed.reply {
                Some(reply) => {
                    std::fs::write(&out, serde_json::to_vec_pretty(&reply)?)?;
                    println!("Signed. Send {} back to the other trustee.", out.display());
                }
                None => {
                    println!("Not signed. If this is what you agreed, run again with --approve.")
                }
            }
        }
        Command::Escrow(EscrowCommand::RecoverFinish {
            backup,
            reply,
            lightwalletd,
            work,
        }) => {
            let work = match work {
                Some(w) => w,
                None => sealforge_cli::recover::default_work_dir(store.root(), &backup)?,
            };
            let reply: sealforge_cli::recover::Reply =
                serde_json::from_slice(&std::fs::read(&reply)?)?;
            let txid =
                sealforge_cli::recover::finish(&backup, &work, &reply, &lightwalletd).await?;
            println!("Broadcast {txid}. Check it's mined before treating the escrow as swept.");
        }
        Command::Escrow(EscrowCommand::RecoverCheck { backups }) => {
            use sealforge_escrow::{
                address::{escrow_address, EncodingParams},
                backup::{drill_sign, Backup},
            };
            let parsed = backups
                .iter()
                .map(|p| {
                    let text = std::fs::read_to_string(p)?;
                    Backup::decode(&text).map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            drill_sign(&parsed, &mut rand::rngs::OsRng)?;
            let first = &parsed[0];
            println!(
                "Recovered: trustees {} of a {}-of-{} escrow signed together with no coordinator, \
                 and the signature verifies under the escrow's key. Escrow address: {}{}",
                parsed
                    .iter()
                    .map(|b| b.id.to_string())
                    .collect::<Vec<_>>()
                    .join(" and "),
                first.t,
                first.n,
                escrow_address(&first.full_viewing_key(), &EncodingParams(first.network))?,
                match first.birthday_height {
                    0 => String::new(),
                    h => format!(" (scan from height {h})"),
                }
            );
        }
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
        AuctionCommand::FundFees { code, zat } => {
            let view = organiser::auction_show(store, url, &code).await?;
            // Before opening, the auction can still be cancelled, and a cancelled
            // escrow isn't scanned: a buffer sent then would be stranded (#40).
            anyhow::ensure!(
                matches!(
                    view.status,
                    sealforge_api::types::AuctionStatus::Open
                        | sealforge_api::types::AuctionStatus::Closed
                ),
                "fund the fee buffer once {code} is open (it is {:?})",
                view.status
            );
            let escrow = view
                .escrow_ua
                .ok_or_else(|| anyhow::anyhow!("{code} has no escrow yet: finish the DKG first"))?;
            let memo = sealforge_core::memo::fee_buffer_memo(&code)?;
            println!(
                "Send {} ZEC to the escrow with this memo, from a shielded wallet:\n\n{}\n",
                sealforge_core::zip321::format_zec(sealforge_core::Zatoshi(zat)),
                sealforge_core::zip321::payment_uri(&escrow, sealforge_core::Zatoshi(zat), &memo)
            );
            println!("Address: {escrow}\nMemo:    {memo}");
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

fn describe(progress: &DkgProgress, code: &str) -> String {
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
        DkgProgress::Verified { escrow_ua } => format!(
            "Escrow address checked against your own group key and attested: {escrow_ua}\n\
             Once the auction opens, run `sealforge trustee check {code}` to confirm bidders see \
             this address, and share the key it prints."
        ),
        DkgProgress::Ready { escrow_ua } => {
            format!("Key generation complete. Escrow address: {escrow_ua}")
        }
    }
}

fn local_dkg(out: &std::path::Path, network: &str) -> anyhow::Result<()> {
    use rand::rngs::OsRng;
    use sealforge_escrow::address::EncodingParams;
    use sealforge_escrow::{address, dkg, Threshold};
    use zcash_protocol::consensus::NetworkType;

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
    let params = EncodingParams(if network == "regtest" {
        NetworkType::Regtest
    } else {
        NetworkType::Test
    });
    let ua = address::escrow_address(&fvk, &params)?;
    let (ufvk, uivk) = address::escrow_view_keys(&fvk, &params)?;
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
