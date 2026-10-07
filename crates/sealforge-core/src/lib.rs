//! sealForge core: pure, deterministic auction logic.
//!
//! Everything here must stay free of I/O and randomness so that the
//! coordinator, the CLI and the WASM auditor compute byte-identical results.
//! See `docs/05-BACKEND-SCHEMA.md` and `docs/02-TRD.md` §2.6.

pub mod amount;
pub mod clearing;
pub mod denominations;
pub mod memo;
pub mod merkle;
pub mod receipt;
pub mod zip321;

pub use amount::Zatoshi;
