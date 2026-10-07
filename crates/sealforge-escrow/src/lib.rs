//! FROST threshold escrow (Milestone 1, task 1.5, critical path).
//!
//! Responsibilities:
//! - Relay DKG round packages between trustees. Secrets never touch the coordinator.
//! - Derive the escrow Ironwood UA and UFVK from the group verifying key.
//! - Build refund / payout PCZTs, collect 2-of-3 signature shares, aggregate, finalise.
//!
//! See docs/02-TRD.md §2.3 and docs/06-IMPLEMENTATION-PLAN.md task 1.5.

use serde::{Deserialize, Serialize};

/// FROST threshold parameters. v1 is fixed at 2-of-3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Threshold {
    pub min_signers: u16,
    pub max_signers: u16,
}

impl Default for Threshold {
    fn default() -> Self {
        Self {
            min_signers: 2,
            max_signers: 3,
        }
    }
}

// TODO(1.5): DKG round 1/2 types wrapping frost-core packages.
// TODO(1.5): escrow address derivation from the group verifying key.
// TODO(3.9): PCZT batch builder + signature aggregation.
