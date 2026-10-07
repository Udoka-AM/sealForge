//! Clearing engine (Milestone 3, task 3.1).
//!
//! Rules (PRD AC5):
//! - Only `valid` bids at or above the reserve take part.
//! - Sort by amount descending, then earlier block height, then lower leaf hash.
//! - Uniform price: the top N bids win and each pays the N-th highest bid.
//! - First price: the top N bids win and each pays its own bid.

use serde::{Deserialize, Serialize};

use crate::Zatoshi;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuctionMode {
    UniformPrice,
    FirstPrice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionRules {
    pub mode: AuctionMode,
    pub quantity: u32,
    pub reserve: Zatoshi,
}

/// A valid bid as seen by the clearing engine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearingBid {
    pub leaf_hash: [u8; 32],
    pub block_height: u32,
    pub amount: Zatoshi,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BidOutcome {
    pub leaf_hash: [u8; 32],
    pub is_winner: bool,
    pub price_paid: Zatoshi,
    pub refund_due: Zatoshi,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearingResult {
    pub clearing_price: Option<Zatoshi>,
    pub total_volume: Zatoshi,
    pub valid_bid_count: u32,
    pub winner_count: u32,
    pub outcomes: Vec<BidOutcome>,
}

/// Compute the clearing result. Must be deterministic for any input order.
pub fn clear(_rules: &AuctionRules, _bids: &[ClearingBid]) -> ClearingResult {
    todo!("Milestone 3, task 3.1: implement clearing per the rules above")
}
