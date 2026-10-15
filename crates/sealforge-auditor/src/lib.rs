//! Public auditor (Milestone 3, tasks 3.4 and 3.5).
//!
//! - [`audit_chain`]: given the published result and every payment decoded
//!   from the chain with the published viewing key (`sealforge-audit` scans
//!   them), reclassify them, recompute the result with the same code the
//!   coordinator used, and compare every published field (PRD AC7). Handles
//!   deposit and commit-reveal auctions.
//! - [`payments_from_published`]: rebuild payments from the published bid list
//!   (the browser's quick check). That checks the published math and root are
//!   consistent, but not that no bid was left out.
//! - [`verify_receipt`]: a bidder checks their receipt against the result (AC8).

use std::collections::BTreeMap;

use sealforge_api::types::{AuctionMode as ApiMode, BidMode, PublishedResult};
use sealforge_core::{
    clearing::{AuctionMode, AuctionRules},
    memo::{BidMemo, RevealMemo},
    merkle::{leaf_hash, verify_proof, MerkleTree},
    receipt::{commitment, price_commitment, Receipt},
    refunds::{refund_dust, split_refund},
    result::{compute_result, ScannedBid},
    selection::{
        included_bids, included_bids_commit_reveal, BidWindow, Payment, RevealPayment, RevealWindow,
    },
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

/// The bidding window the published result was computed over.
fn bid_window(published: &PublishedResult) -> BidWindow {
    BidWindow {
        auction_code: published.auction_code.clone(),
        open_height: published.scan_from_height,
        close_height: published
            .rules
            .close_height
            .unwrap_or(published.scan_to_height),
        reserve: Zatoshi(published.rules.reserve_zat),
    }
}

/// A payment exactly as the chain shows it to the viewing key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainPayment {
    #[serde(with = "hex::serde")]
    pub txid: [u8; 32],
    pub output_index: u32,
    pub block_height: u32,
    pub amount_zat: u64,
    pub memo: Option<String>,
}

/// Audit from chain payments, in whichever bid mode the auction used. Memos
/// are decoded here, the same way the coordinator's scanner decodes them.
pub fn audit_chain(published: &PublishedResult, payments: &[ChainPayment]) -> AuditReport {
    let deposits: Vec<Payment> = payments
        .iter()
        .map(|p| Payment {
            txid: p.txid,
            output_index: p.output_index,
            block_height: p.block_height,
            amount: Zatoshi(p.amount_zat),
            memo: p.memo.as_deref().and_then(|m| BidMemo::decode(m).ok()),
        })
        .collect();
    if published.rules.bid_mode == BidMode::Deposit {
        return audit(published, &deposits);
    }
    let reveals: Vec<RevealPayment> = payments
        .iter()
        .filter_map(|p| {
            Some(RevealPayment {
                txid: p.txid,
                output_index: p.output_index,
                block_height: p.block_height,
                amount: Zatoshi(p.amount_zat),
                memo: RevealMemo::decode(p.memo.as_deref()?).ok()?,
            })
        })
        .collect();
    audit_commit_reveal(published, &deposits, &reveals)
}

/// Recompute a commit-reveal result from deposits and reveals on chain.
pub fn audit_commit_reveal(
    published: &PublishedResult,
    deposits: &[Payment],
    reveals: &[RevealPayment],
) -> AuditReport {
    let window = RevealWindow {
        bid: bid_window(published),
        reveal_close_height: published.scan_to_height,
    };
    compare(
        published,
        &included_bids_commit_reveal(&window, deposits, reveals),
    )
}

/// Recompute the result from chain payments (deposit mode) and compare it with
/// what was published.
pub fn audit(published: &PublishedResult, payments: &[Payment]) -> AuditReport {
    compare(published, &included_bids(&bid_window(published), payments))
}

fn compare(published: &PublishedResult, included: &[ScannedBid]) -> AuditReport {
    let rules = AuctionRules {
        mode: match published.rules.mode {
            ApiMode::UniformPrice => AuctionMode::UniformPrice,
            ApiMode::FirstPrice => AuctionMode::FirstPrice,
        },
        quantity: published.rules.quantity,
        reserve: Zatoshi(published.rules.reserve_zat),
    };

    let mut fields = Vec::new();
    match compute_result(&rules, included) {
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

fn hex32(value: &str) -> Option<[u8; 32]> {
    hex::decode(value).ok()?.try_into().ok()
}

/// Rebuild payments from the published bid list (consistency check only: a
/// coordinator that left a bid out of the list would not be caught this way).
pub fn payments_from_published(published: &PublishedResult) -> Result<Vec<Payment>, String> {
    published
        .bids
        .iter()
        .map(|b| {
            Ok(Payment {
                txid: hex32(&b.txid).ok_or("bad txid in published result")?,
                output_index: b.output_index,
                block_height: b.block_height,
                amount: Zatoshi(b.amount_zat),
                memo: Some(BidMemo {
                    auction_code: published.auction_code.clone(),
                    commitment: hex32(&b.commitment).ok_or("bad commitment in published result")?,
                    refund_ua: String::new(),
                }),
            })
        })
        .collect()
}

/// What a bidder learns from checking their receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptCheck {
    /// The receipt's salt and refund address reproduce its commitment.
    pub receipt_valid: bool,
    /// A bid with this commitment is in the published result.
    pub found: bool,
    /// The bid's leaf is in the tree under the published Merkle root.
    pub included: bool,
    /// The amount on chain equals the amount in the receipt.
    pub amount_matches: bool,
    pub amount_zat: Option<u64>,
    pub block_height: Option<u32>,
    pub is_winner: bool,
    pub price_paid_zat: u64,
    pub refund_due_zat: u64,
    /// How the refund will arrive (canonical denominations).
    pub refund_notes_zat: Vec<u64>,
    /// Part of the refund below 0.0001 ZEC, which is not sent (worth less than the fee).
    pub dust_zat: u64,
}

/// Check a receipt against a published result.
pub fn verify_receipt(published: &PublishedResult, receipt: &Receipt) -> ReceiptCheck {
    // Commit-reveal receipts commit to the hidden price as well.
    let expected = match receipt.price_zat {
        Some(price) => price_commitment(
            &receipt.auction_code,
            &receipt.refund_ua,
            price,
            &receipt.salt,
        ),
        None => commitment(&receipt.auction_code, &receipt.refund_ua, &receipt.salt),
    };
    let receipt_valid =
        receipt.auction_code == published.auction_code && expected == receipt.commitment;
    let wanted = hex::encode(receipt.commitment);
    let bid = published.bids.iter().find(|b| b.commitment == wanted);

    let mut check = ReceiptCheck {
        receipt_valid,
        found: bid.is_some(),
        included: false,
        amount_matches: false,
        amount_zat: bid.map(|b| b.amount_zat),
        block_height: bid.map(|b| b.block_height),
        is_winner: false,
        price_paid_zat: 0,
        refund_due_zat: 0,
        refund_notes_zat: Vec::new(),
        dust_zat: 0,
    };
    let Some(bid) = bid else { return check };

    let leaf = match hex32(&bid.txid) {
        Some(txid) => leaf_hash(&txid, bid.output_index, bid.amount_zat, &receipt.commitment),
        None => return check,
    };
    let leaves: Option<Vec<[u8; 32]>> =
        published.bids.iter().map(|b| hex32(&b.leaf_hash)).collect();
    let root = hex32(&published.merkle_root);
    if let (Some(leaves), Some(root)) = (leaves, root) {
        if let Ok(tree) = MerkleTree::from_leaves(leaves) {
            check.included = hex::encode(leaf) == bid.leaf_hash
                && tree.root() == root
                && tree
                    .proof(&leaf)
                    .is_ok_and(|proof| verify_proof(&root, &leaf, &proof));
        }
    }
    // Commit-reveal: the bid is the revealed price, carried by the deposit.
    check.amount_matches = match receipt.price_zat {
        Some(price) => bid.amount_zat == price && bid.deposit_zat == Some(receipt.amount_zat),
        None => bid.amount_zat == receipt.amount_zat,
    };
    check.is_winner = bid.is_winner;
    check.price_paid_zat = bid.price_paid_zat;
    check.refund_due_zat = bid.refund_due_zat;
    check.refund_notes_zat = split_refund(Zatoshi(bid.refund_due_zat))
        .into_iter()
        .map(|z| z.0)
        .collect();
    check.dust_zat = refund_dust(Zatoshi(bid.refund_due_zat)).0;
    check
}

/// JSON in, JSON out bindings for the web app (task 3.5).
#[cfg(feature = "wasm")]
mod wasm {
    use wasm_bindgen::prelude::*;

    use super::*;

    fn parse<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> Result<T, JsError> {
        serde_json::from_str(json).map_err(|e| JsError::new(&format!("invalid {what}: {e}")))
    }

    fn to_json(value: &impl Serialize) -> Result<String, JsError> {
        serde_json::to_string(value).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Version string, so the audit page can show which auditor build ran.
    #[wasm_bindgen]
    pub fn auditor_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Consistency audit of a published result against its own bid list.
    #[wasm_bindgen]
    pub fn audit_published(published_json: &str) -> Result<String, JsError> {
        let published: PublishedResult = parse(published_json, "published result")?;
        let payments = payments_from_published(&published).map_err(|e| JsError::new(&e))?;
        to_json(&audit(&published, &payments))
    }

    /// Full audit with payments decoded from the chain (used once the scan lands).
    #[wasm_bindgen]
    pub fn audit_with_payments(
        published_json: &str,
        payments_json: &str,
    ) -> Result<String, JsError> {
        let published: PublishedResult = parse(published_json, "published result")?;
        let payments: Vec<ChainPayment> = parse(payments_json, "payments")?;
        to_json(&audit_chain(&published, &payments))
    }

    /// Check a bidder's receipt against a published result.
    #[wasm_bindgen]
    pub fn check_receipt(published_json: &str, receipt_json: &str) -> Result<String, JsError> {
        let published: PublishedResult = parse(published_json, "published result")?;
        let receipt: Receipt = parse(receipt_json, "receipt")?;
        to_json(&verify_receipt(&published, &receipt))
    }
}

#[cfg(test)]
mod tests {
    use sealforge_api::types::{Network, ResultBid, ResultRules};
    use sealforge_core::memo::BidMemo;

    use super::*;

    const CODE: &str = "7K3QX9MZ2P";

    const REFUND: &str = "utest1refund";

    fn salt(i: u8) -> [u8; 32] {
        [i; 32]
    }

    /// Payments whose commitments come from real receipts (salt i, shared refund UA).
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
                commitment: commitment(CODE, REFUND, &salt(i)),
                refund_ua: REFUND.into(),
            }),
        })
        .collect()
    }

    fn receipt(i: u8, amount_zat: u64) -> Receipt {
        Receipt {
            version: 1,
            auction_code: CODE.into(),
            amount_zat,
            refund_ua: REFUND.into(),
            salt: salt(i),
            commitment: commitment(CODE, REFUND, &salt(i)),
            price_zat: None,
        }
    }

    #[test]
    fn consistency_audit_from_the_published_list() {
        let published = honest();
        let payments = payments_from_published(&published).unwrap();
        assert!(audit(&published, &payments).matches);
    }

    #[test]
    fn winning_receipt_is_included_with_its_refund_plan() {
        let check = verify_receipt(&honest(), &receipt(1, 5_000_000));
        assert!(check.receipt_valid && check.found && check.included && check.amount_matches);
        assert!(check.is_winner);
        assert_eq!(
            check.price_paid_zat, 4_000_000,
            "uniform price: lowest winning bid of 2 lots"
        );
        assert_eq!(check.refund_due_zat, 1_000_000);
        assert_eq!(
            check.refund_notes_zat.iter().sum::<u64>() + check.dust_zat,
            1_000_000
        );
    }

    #[test]
    fn losing_and_unknown_receipts() {
        let lost = verify_receipt(&honest(), &receipt(2, 3_000_000));
        assert!(lost.included && !lost.is_winner);
        assert_eq!(lost.refund_due_zat, 3_000_000);

        let unknown = verify_receipt(&honest(), &receipt(9, 1));
        assert!(unknown.receipt_valid && !unknown.found && !unknown.included);

        let mut forged = receipt(1, 5_000_000);
        forged.refund_ua = "utest1someoneelse".into();
        assert!(
            !verify_receipt(&honest(), &forged).receipt_valid,
            "salt must reproduce the commitment"
        );
    }

    #[test]
    fn tampered_root_breaks_inclusion() {
        let mut published = honest();
        published.merkle_root = hex::encode([0u8; 32]);
        assert!(!verify_receipt(&published, &receipt(1, 5_000_000)).included);
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
        let payments = payments();
        let r = compute_result(&rules, &included_bids(&window, &payments)).unwrap();
        let by_leaf: BTreeMap<_, _> = payments
            .iter()
            .map(|p| {
                let c = p.memo.as_ref().unwrap().commitment;
                (leaf_hash(&p.txid, p.output_index, p.amount.0, &c), p)
            })
            .collect();
        PublishedResult {
            auction_code: CODE.into(),
            network: Network::Testnet,
            rules: ResultRules {
                mode: ApiMode::UniformPrice,
                quantity: 2,
                reserve_zat: 1_000_000,
                bid_mode: sealforge_api::types::BidMode::Deposit,
                close_height: None,
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
                    txid: hex::encode(by_leaf[&o.leaf_hash].txid),
                    output_index: by_leaf[&o.leaf_hash].output_index,
                    block_height: by_leaf[&o.leaf_hash].block_height,
                    amount_zat: by_leaf[&o.leaf_hash].amount.0,
                    deposit_zat: None,
                    commitment: hex::encode(
                        by_leaf[&o.leaf_hash].memo.as_ref().unwrap().commitment,
                    ),
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

#[cfg(test)]
mod commit_reveal_tests {
    use sealforge_api::types::{BidMode, Network, ResultBid, ResultRules};
    use sealforge_core::{memo::RevealMemo, receipt::price_commitment};

    use super::*;

    const CODE: &str = "CR4DT00001";
    const REFUND: &str = "utest1refund";

    fn salt(i: u8) -> [u8; 32] {
        [i; 32]
    }

    /// (deposit payment, reveal payment) for bidder `i`.
    fn bidder(i: u8, deposit: u64, price: u64) -> (ChainPayment, ChainPayment) {
        let c = price_commitment(CODE, REFUND, price, &salt(i));
        let deposit = ChainPayment {
            txid: [i; 32],
            output_index: 0,
            block_height: 150 + u32::from(i),
            amount_zat: deposit,
            memo: Some(
                BidMemo {
                    auction_code: CODE.into(),
                    commitment: c,
                    refund_ua: REFUND.into(),
                }
                .encode()
                .unwrap(),
            ),
        };
        let reveal = ChainPayment {
            txid: [100 + i; 32],
            output_index: 0,
            block_height: 250 + u32::from(i),
            amount_zat: 10_000,
            memo: Some(
                RevealMemo {
                    auction_code: CODE.into(),
                    commitment: c,
                    price_zat: price,
                    salt: salt(i),
                }
                .encode()
                .unwrap(),
            ),
        };
        (deposit, reveal)
    }

    fn chain() -> Vec<ChainPayment> {
        let mut out = Vec::new();
        for (i, deposit, price) in [
            (1, 100_000_000, 42_000_000),
            (2, 50_000_000, 30_000_000),
            (3, 50_000_000, 10_000_000),
        ] {
            let (d, r) = bidder(i, deposit, price);
            out.push(d);
            out.push(r);
        }
        out
    }

    /// What an honest coordinator publishes for `chain()`.
    fn published() -> PublishedResult {
        let rules = AuctionRules {
            mode: AuctionMode::UniformPrice,
            quantity: 2,
            reserve: Zatoshi(1_000_000),
        };
        let mut p = PublishedResult {
            auction_code: CODE.into(),
            network: Network::Testnet,
            rules: ResultRules {
                mode: ApiMode::UniformPrice,
                quantity: 2,
                reserve_zat: 1_000_000,
                bid_mode: BidMode::CommitReveal,
                close_height: Some(200),
            },
            scan_from_height: 100,
            scan_to_height: 344,
            uivk: String::new(),
            algorithm_version: String::new(),
            clearing_price_zat: None,
            total_volume_zat: 0,
            valid_bid_count: 0,
            winner_count: 0,
            merkle_root: String::new(),
            bids: vec![],
        };
        let deposits: Vec<Payment> = chain()
            .iter()
            .map(|c| Payment {
                txid: c.txid,
                output_index: c.output_index,
                block_height: c.block_height,
                amount: Zatoshi(c.amount_zat),
                memo: c.memo.as_deref().and_then(|m| BidMemo::decode(m).ok()),
            })
            .collect();
        let reveals: Vec<RevealPayment> = chain()
            .iter()
            .filter_map(|c| {
                Some(RevealPayment {
                    txid: c.txid,
                    output_index: c.output_index,
                    block_height: c.block_height,
                    amount: Zatoshi(c.amount_zat),
                    memo: RevealMemo::decode(c.memo.as_deref()?).ok()?,
                })
            })
            .collect();
        let window = RevealWindow {
            bid: bid_window(&p),
            reveal_close_height: 344,
        };
        let included = included_bids_commit_reveal(&window, &deposits, &reveals);
        let r = compute_result(&rules, &included).unwrap();
        p.algorithm_version = r.algorithm_version.clone();
        p.clearing_price_zat = r.clearing.clearing_price.map(|x| x.0);
        p.total_volume_zat = r.clearing.total_volume.0;
        p.valid_bid_count = r.clearing.valid_bid_count;
        p.winner_count = r.clearing.winner_count;
        p.merkle_root = hex::encode(r.merkle_root);
        p.bids = included
            .iter()
            .map(|b| {
                let o = r
                    .clearing
                    .outcomes
                    .iter()
                    .find(|o| o.leaf_hash == b.leaf_hash())
                    .unwrap();
                let deposit = deposits.iter().find(|d| d.txid == b.txid).unwrap().amount.0;
                ResultBid {
                    txid: hex::encode(b.txid),
                    output_index: b.output_index,
                    block_height: b.block_height,
                    amount_zat: b.amount.0,
                    deposit_zat: Some(deposit),
                    commitment: hex::encode(b.commitment),
                    leaf_hash: hex::encode(o.leaf_hash),
                    is_winner: o.is_winner,
                    price_paid_zat: o.price_paid.0,
                    refund_due_zat: o.refund_due.0,
                }
            })
            .collect();
        p
    }

    #[test]
    fn chain_audit_reproduces_a_commit_reveal_result() {
        let p = published();
        assert_eq!(p.clearing_price_zat, Some(30_000_000));
        let report = audit_chain(&p, &chain());
        assert!(report.matches, "{:?}", report.fields);
        // The quick browser check works on the published (revealed) prices too.
        assert!(audit(&p, &payments_from_published(&p).unwrap()).matches);
    }

    #[test]
    fn a_suppressed_reveal_is_caught() {
        // A coordinator that ignored bidder 1's reveal publishes a different result.
        let mut chain = chain();
        let honest = audit_chain(&published(), &chain);
        assert!(honest.matches);
        chain.retain(|c| c.txid != [101; 32]);
        assert!(!audit_chain(&published(), &chain).matches);
    }

    #[test]
    fn receipts_with_a_hidden_price_verify() {
        let p = published();
        let receipt = Receipt {
            version: 2,
            auction_code: CODE.into(),
            amount_zat: 100_000_000,
            refund_ua: REFUND.into(),
            salt: salt(1),
            commitment: price_commitment(CODE, REFUND, 42_000_000, &salt(1)),
            price_zat: Some(42_000_000),
        };
        let check = verify_receipt(&p, &receipt);
        assert!(check.receipt_valid && check.found && check.included && check.amount_matches);
        assert!(check.is_winner);
        assert_eq!(check.price_paid_zat, 30_000_000);
        let lied = Receipt {
            price_zat: Some(41_000_000),
            ..receipt
        };
        assert!(!verify_receipt(&p, &lied).receipt_valid);
    }
}
