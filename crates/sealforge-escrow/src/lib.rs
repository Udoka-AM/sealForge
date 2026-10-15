//! FROST 2-of-3 threshold escrow (Milestone 1, task 1.5, critical path).
//!
//! - [`dkg`]: the three DKG rounds each trustee runs locally. The coordinator
//!   only relays packages, and signing shares never leave a trustee's machine.
//! - [`address`]: derives the escrow's Orchard/Ironwood full viewing key from
//!   the FROST group key (`ak`), plus its Unified Address, UFVK and the UIVK
//!   that is published at close.
//! - [`signing`]: rerandomized FROST signing of a PCZT action's spend
//!   authorization (sighash + α), producing a standard RedPallas `SpendAuth`
//!   signature for `pczt::roles::signer::Signer::apply_ironwood_signature`.
//!
//! Ironwood (NU6.3) reuses Orchard keys and receivers, so the escrow is an
//! Orchard-key wallet whose notes live in the Ironwood pool.

#[cfg(feature = "address")]
pub mod address;
pub mod dkg;
pub mod signing;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// FROST(Pallas, BLAKE2b-512) with Orchard's even-Y group key normalisation.
pub use reddsa_frost::frost::redpallas as frost;

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

#[derive(Debug, Error)]
pub enum EscrowError {
    #[error("FROST error: {0}")]
    Frost(#[from] frost::Error),
    #[error("could not derive a valid escrow viewing key")]
    KeyDerivation,
    #[error("invalid randomizer (alpha)")]
    InvalidRandomizer,
    #[error("unified encoding error: {0}")]
    Encoding(String),
    #[error("signature does not verify against the randomized spend key")]
    BadSignature,
    #[error("batch values don't line up with its spends")]
    BatchShape,
}
