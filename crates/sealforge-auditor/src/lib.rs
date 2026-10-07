//! Public auditor (Milestone 3, tasks 3.4 and 3.5).
//!
//! Input: published IVK, auction params, block range.
//! Output: clearing price, volume, winners and Merkle root. These must equal the
//! published values byte-for-byte (PRD AC7).

use serde::{Deserialize, Serialize};

/// What the organiser publishes at close.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedAuction {
    pub auction_code: String,
    pub ivk: String,
    pub scan_from_height: u32,
    pub scan_to_height: u32,
    pub rules: sealforge_core::clearing::AuctionRules,
    pub merkle_root: String,
}

/// Field-by-field comparison shown on the audit page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub matches: bool,
    pub fields: Vec<FieldCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldCheck {
    pub field: String,
    pub published: String,
    pub recomputed: String,
    pub ok: bool,
}

// TODO(3.4): decrypt compact blocks with the IVK, parse memos via sealforge_core::memo,
// run sealforge_core::clearing::clear, build the Merkle root, compare.

#[cfg(feature = "wasm")]
mod wasm {
    use wasm_bindgen::prelude::*;

    /// Version string, so the audit page can show which auditor build ran.
    #[wasm_bindgen]
    pub fn auditor_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}
