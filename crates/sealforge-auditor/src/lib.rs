//! Public auditor (Milestone 3, tasks 3.4 and 3.5).
//!
//! [`audit`] is the pure half: given the published result and the payments
//! decoded from the chain with the published viewing key, it reclassifies the
//! payments, recomputes the result with the same code the coordinator used,
//! and compares every published field (PRD AC7). Scanning the chain is task 3.4.

use std::collections::BTreeMap;

use sealforge_api::types::{AuctionMode as ApiMode, PublishedResult};
use sealforge_core::{
    clearing::{AuctionMode, AuctionRules},
    result::compute_result,
    selection::{included_bids, BidWindow, Payment},
    Zatoshi,
};
use serde::{Deserialize, Serialize};

/// Field-by-field comparison shown on the audit page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    pub matches: bool,
    pub fields: Vec<FieldCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldCheck {
    pub field: String,
    pub published: String,
    pub recomputed: String,
    pub ok: bool,
}

fn check(fields: &mut Vec<FieldCheck>, field: &str, published: String, recomputed: String) {
    fields.push(FieldCheck {
        field: field.to_owned(),
        ok: published == recomputed,
        published,
        recomputed,
    });
}

fn opt(v: Option<u64>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| "none".into())
}

/// Recompute the result from chain payments and compare it with what was published.
pub fn audit(published: &PublishedResult, payments: &[Payment]) -> AuditReport {
    let window = BidWindow {
        auction_code: published.auction_code.clone(),
        open_height: published.scan_from_height,
        close_height: published.scan_to_height,
        reserve: Zatoshi(published.rules.reserve_zat),
    };
    let rules = AuctionRules {
        mode: match published.rules.mode {
            ApiMode::UniformPrice => AuctionMode::UniformPrice,
            ApiMode::FirstPrice => AuctionMode::FirstPrice,
        },
        quantity: published.rules.quantity,
        reserve: Zatoshi(published.rules.reserve_zat),
    };

    let mut fields = Vec::new();
    match compute_result(&rules, &included_bids(&window, payments)) {
        Err(err) => check(
            &mut fields,
            "result",
            "computable".into(),
            format!("error: {err}"),
        ),
        Ok(r) => {
            let c = &r.clearing;
            check(
                &mut fields,
                "clearing_price_zat",
                opt(published.clearing_price_zat),
                opt(c.clearing_price.map(|p| p.0)),
            );
            check(
                &mut fields,
                "total_volume_zat",
                published.total_volume_zat.to_string(),
                c.total_volume.0.to_string(),
            );
            check(
                &mut fields,
                "valid_bid_count",
                published.valid_bid_count.to_string(),
                c.valid_bid_count.to_string(),
            );
            check(
                &mut fields,
                "winner_count",
                published.winner_count.to_string(),
                c.winner_count.to_string(),
            );
            check(
                &mut fields,
                "merkle_root",
                published.merkle_root.clone(),
                hex::encode(r.merkle_root),
            );

            // Every published per-bid outcome must match the recomputed one.
            let recomputed: BTreeMap<String, _> = c
                .outcomes
                .iter()
                .map(|o| {
                    (
                        hex::encode(o.leaf_hash),
                        (o.is_winner, o.price_paid.0, o.refund_due.0),
                    )
                })
                .collect();
            let mismatched = published
                .bids
                .iter()
                .filter(|b| {
                    recomputed.get(&b.leaf_hash)
                        != Some(&(b.is_winner, b.price_paid_zat, b.refund_due_zat))
                })
                .count()
                + recomputed.len().abs_diff(published.bids.len());
            check(
                &mut fields,
                "bid_outcomes_mismatched",
                "0".into(),
                mismatched.to_string(),
            );
        }
    }
    AuditReport {
        matches: fields.iter().all(|f| f.ok),
        fields,
    }
}

#[cfg(feature = "wasm")]
mod wasm {
    use wasm_bindgen::prelude::*;

    /// Version string, so the audit page can show which auditor build ran.
    #[wasm_bindgen]
    pub fn auditor_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

#[cfg(test)]
mod tests {
    use sealforge_api::types::{Network, ResultBid, ResultRules};
    use sealforge_core::memo::BidMemo;

    use super::*;

    const CODE: &str = "7K3QX9MZ2P";

    fn payments() -> Vec<Payment> {
        [
            (1u8, 5_000_000u64),
            (2, 3_000_000),
            (3, 4_000_000),
            (4, 500_000),
        ]
        .into_iter()
        .map(|(i, amount)| Payment {
            txid: [i; 32],
            output_index: 0,
            block_height: 150,
            amount: Zatoshi(amount),
            memo: Some(BidMemo {
                auction_code: CODE.into(),
                commitment: [i; 32],
                refund_ua: "utest1refund".into(),
            }),
        })
        .collect()
    }

    /// What an honest coordinator would publish for `payments()`.
    fn honest() -> PublishedResult {
        let rules = AuctionRules {
            mode: AuctionMode::UniformPrice,
            quantity: 2,
            reserve: Zatoshi(1_000_000),
        };
        let window = BidWindow {
            auction_code: CODE.into(),
            open_height: 100,
            close_height: 200,
            reserve: rules.reserve,
        };
        let r = compute_result(&rules, &included_bids(&window, &payments())).unwrap();
        PublishedResult {
            auction_code: CODE.into(),
            network: Network::Testnet,
            rules: ResultRules {
                mode: ApiMode::UniformPrice,
                quantity: 2,
                reserve_zat: 1_000_000,
            },
            scan_from_height: 100,
            scan_to_height: 200,
            uivk: "uivktest1…".into(),
            algorithm_version: r.algorithm_version.clone(),
            clearing_price_zat: r.clearing.clearing_price.map(|p| p.0),
            total_volume_zat: r.clearing.total_volume.0,
            valid_bid_count: r.clearing.valid_bid_count,
            winner_count: r.clearing.winner_count,
            merkle_root: hex::encode(r.merkle_root),
            bids: r
                .clearing
                .outcomes
                .iter()
                .map(|o| ResultBid {
                    leaf_hash: hex::encode(o.leaf_hash),
                    is_winner: o.is_winner,
                    price_paid_zat: o.price_paid.0,
                    refund_due_zat: o.refund_due.0,
                })
                .collect(),
        }
    }

    #[test]
    fn honest_result_matches() {
        let report = audit(&honest(), &payments());
        assert!(report.matches, "{report:#?}");
    }

    #[test]
    fn inflated_volume_or_hidden_bid_is_caught() {
        let mut inflated = honest();
        inflated.total_volume_zat += 1;
        assert!(!audit(&inflated, &payments()).matches);

        // Coordinator "forgets" a bid: the chain still shows it.
        let mut fewer = payments();
        fewer.pop();
        let report = audit(&honest(), &fewer);
        assert!(!report.matches);
        assert!(report
            .fields
            .iter()
            .any(|f| f.field == "merkle_root" && !f.ok));
    }

    #[test]
    fn rewritten_outcome_is_caught() {
        let mut cheated = honest();
        cheated.bids[0].refund_due_zat = 0;
        let report = audit(&cheated, &payments());
        assert!(!report.matches);
        assert!(report
            .fields
            .iter()
            .any(|f| f.field == "bid_outcomes_mismatched" && !f.ok));
    }
}
