//! `sealforge-linkability`: run the harness and write the evidence files.
//!
//! cargo run -p sealforge-linkability --release -- --out docs/evidence

use std::path::PathBuf;

use clap::Parser;
use sealforge_linkability::{run, Params, ScenarioResult};

#[derive(Debug, Parser)]
#[command(about = "Measure refund linkability: naive refunds vs sealForge")]
struct Args {
    #[arg(long, default_value_t = 500)]
    bidders: usize,
    #[arg(long, default_value_t = 50)]
    quantity: u32,
    #[arg(long, default_value_t = 20)]
    trials: u64,
    #[arg(long, default_value_t = 7)]
    seed: u64,
    /// Directory for linkability.json and linkability.svg. Prints only if omitted.
    #[arg(long)]
    out: Option<PathBuf>,
}

fn svg(results: &[ScenarioResult], params: &Params) -> String {
    let (w, row, left, bar_max) = (720.0, 56.0, 230.0, 420.0);
    let h = 90.0 + row * results.len() as f64 + 40.0;
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="IBM Plex Sans, system-ui, sans-serif" role="img" aria-labelledby="title desc">
<title id="title">Refunds an exit observer can link to a bid</title>
<desc id="desc">{desc}</desc>
<rect width="100%" height="100%" fill="#0B0D10"/>
<text x="24" y="36" fill="#ECEAE4" font-size="18" font-weight="600">Refunds an exit observer can link to a bid</text>
<text x="24" y="58" fill="#9AA0A9" font-size="12">{} bidders, {} lots, {} seeded auctions each. Lower is better.</text>
"##,
        params.bidders,
        params.quantity,
        params.trials,
        desc = results
            .iter()
            .map(|r| format!("{}: {:.1}% linked", r.label, r.linked_share * 100.0))
            .collect::<Vec<_>>()
            .join("; "),
    );
    for (i, r) in results.iter().enumerate() {
        let y = 90.0 + row * i as f64;
        let len = (r.linked_share * bar_max).max(2.0);
        let color = if r.linked_share > 0.5 {
            "#E5484D"
        } else if r.linked_share > 0.05 {
            "#E5A50A"
        } else {
            "#3FB37F"
        };
        out.push_str(&format!(
            r##"<text x="24" y="{ty}" fill="#ECEAE4" font-size="13">{label}</text>
<text x="24" y="{sy}" fill="#9AA0A9" font-size="11">{per:.1} notes per refund</text>
<rect x="{left}" y="{y}" width="{len:.1}" height="24" rx="3" fill="{color}"/>
<text x="{tx:.1}" y="{ly}" fill="#ECEAE4" font-size="13" font-family="IBM Plex Mono, monospace">{pct:.1}%</text>
"##,
            ty = y + 12.0,
            sy = y + 28.0,
            label = r.label,
            per = r.notes_per_refund,
            tx = left + len + 8.0,
            ly = y + 17.0,
            pct = r.linked_share * 100.0,
        ));
    }
    out.push_str("</svg>\n");
    out
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let params = Params {
        bidders: args.bidders,
        quantity: args.quantity,
        trials: args.trials,
        seed: args.seed,
        ..Params::default()
    };
    let results = run(&params);
    for r in &results {
        println!(
            "{:<28} {:>6.1}% linked   {:.1} notes/refund",
            r.label,
            r.linked_share * 100.0,
            r.notes_per_refund
        );
    }
    if let Some(dir) = args.out {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("linkability.json"),
            serde_json::to_string_pretty(
                &serde_json::json!({ "params": params, "results": results }),
            )? + "\n",
        )?;
        std::fs::write(dir.join("linkability.svg"), svg(&results, &params))?;
        println!("Wrote {}/linkability.{{json,svg}}", dir.display());
    }
    Ok(())
}
