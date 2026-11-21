//! What a trustee checks before co-signing an escrow transaction (security
//! review D1).
//!
//! The coordinator builds every refund, payout and consolidation as a PCZT.
//! A trustee used to sign only its sighash. Now it decodes the PCZT itself and
//! signs only what it has checked:
//!
//! 1. The shared co-signer checks of frost-zec (spec § 6), vendored in
//!    [`frost_zec`]: every spend is the escrow's own, every output decrypts,
//!    nothing is hidden, the fee and action count are bounded, the expiry is
//!    near, and the sighash is computed here, not taken from the coordinator.
//! 2. sealForge's own rule: every payment is owed. See [`plan`].

pub mod frost_zec;
pub mod plan;

pub use plan::{verify_batch, BatchKind, BatchRules, Refusal, VerifiedBatch};

/// The shared refusal class (frost-zec `vectors/tamper/classes.md`) of an error
/// from [`frost_zec::check`]. An error that maps to none is `"unclassified"`,
/// and the tamper runner fails on it: every refusal must have a class.
/// Mapping as in frost-zec's runner at c417ced.
pub fn refusal_class(error: &str) -> &'static str {
    const MAP: [(&str, &str); 14] = [
        ("does not decode", "foreign-pool"),
        ("spent note", "foreign-spend"),
        ("outgoing viewing key", "unreadable-output"),
        ("cannot decrypt", "unreadable-output"),
        ("zero-value note to another address", "hidden-output"),
        ("more than ZIP 317", "fee"),
        ("extra ones move no value", "padding"),
        ("far beyond", "expiry"),
        ("never expires", "expiry"),
        ("not the one it pays", "payment"),
        ("randomizer", "structure"),
        ("dummy", "structure"),
        ("anchor", "structure"),
        ("commitment", "structure"),
    ];
    MAP.iter()
        .find(|(text, _)| error.contains(text))
        .map(|(_, class)| *class)
        .unwrap_or("unclassified")
}
