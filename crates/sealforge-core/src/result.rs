//! The single entry point for computing an auction result from scanned bids.
//!
//! The coordinator (at close) and the auditor (CLI and WASM) both call
//! [`compute_result`], so a published result can be recomputed byte-for-byte
//! (PRD AC7).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    clearing::{self, AuctionRules, ClearingBid, ClearingError, ClearingResult},
    merkle::{self, MerkleError, MerkleTree},
    Zatoshi,
};

/// A valid bid decoded from the escrow by the scanner or the auditor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedBid {
    #[serde(with = "hex::serde")]
    pub txid: [u8; 32],
    pub output_index: u32,
    pub block_height: u32,
    pub amount: Zatoshi,
    #[serde(with = "hex::serde")]
    pub commitment: [u8; 32],
}

impl ScannedBid {
    pub fn leaf_hash(&self) -> merkle::Hash {
        merkle::leaf_hash(
            &self.txid,
            self.output_index,
            self.amount.0,
            &self.commitment,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionResult {
    pub algorithm_version: String,
    pub clearing: ClearingResult,
    #[serde(with = "hex::serde")]
    pub merkle_root: [u8; 32],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResultError {
    #[error(transparent)]
    Clearing(#[from] ClearingError),
    #[error(transparent)]
    Merkle(#[from] MerkleError),
}

/// Compute clearing and the Merkle root over every valid bid (including bids
/// below the reserve, so every bidder can prove inclusion and claim a refund).
pub fn compute_result(
    rules: &AuctionRules,
    bids: &[ScannedBid],
) -> Result<AuctionResult, ResultError> {
    let clearing_bids: Vec<ClearingBid> = bids
        .iter()
        .map(|b| ClearingBid {
            leaf_hash: b.leaf_hash(),
            block_height: b.block_height,
            amount: b.amount,
        })
        .collect();
    let clearing = clearing::clear(rules, &clearing_bids)?;
    let tree = MerkleTree::from_leaves(clearing_bids.iter().map(|b| b.leaf_hash).collect())?;
    Ok(AuctionResult {
        algorithm_version: clearing::ALGORITHM_VERSION.to_owned(),
        clearing,
        merkle_root: tree.root(),
    })
}
