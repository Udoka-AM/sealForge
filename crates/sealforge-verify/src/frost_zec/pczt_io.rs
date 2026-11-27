// Vendored from frost-zec (https://github.com/OpenPGP-Wallet/frost-zec) at commit
// c417ced, MIT License, Copyright (c) 2026 Khadim. See ../../LICENSE-frost-zec.
// Kept as close to upstream as possible so the shared signer checks (frost-zec
// spec § 6) and tamper cases (§ 8.2, frost-zec#7) stay comparable. Local
// changes are marked "sealForge:".

//! Reading a PCZT, finding the spends to authorize, and inserting signatures.
//!
//! PCZTs come from zcash-devtool (`pczt create`, then `pczt prove`) and go
//! back to it once signed (`pczt send`).

use std::error::Error;
use std::fmt;

use pczt::roles::low_level_signer::OrchardParseError;
use pczt::Pczt;
use zcash_protocol::constants::V6_TX_VERSION;

/// Errors raised inside the low-level signer callbacks.
#[derive(Debug)]
pub enum TxError {
    /// The shielded bundle could not be parsed.
    Parse(OrchardParseError),
    /// The PCZT is not one this code signs.
    Refused(String),
}

impl From<OrchardParseError> for TxError {
    fn from(error: OrchardParseError) -> Self {
        TxError::Parse(error)
    }
}

impl fmt::Display for TxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxError::Parse(error) => write!(f, "cannot parse the shielded bundle: {error:?}"),
            TxError::Refused(reason) => write!(f, "{reason}"),
        }
    }
}

impl Error for TxError {}

/// Reads a PCZT.
pub fn parse(bytes: &[u8]) -> Result<Pczt, Box<dyn Error>> {
    Pczt::parse(bytes).map_err(|e| format!("not a valid PCZT: {e:?}").into())
}

/// The sighash every spend authorization signature commits to. Only v6
/// transactions are accepted: a v5 sighash over a v6 transaction would sign
/// fine here and only fail at broadcast (see zcash-sign, frost-tools#593).
///
/// sealForge: computed with pczt's Signer role (`shielded_sighash`), as
/// sealforge-chain does when it builds the PCZT, rather than
/// `PrecomputedSighashData`, so the trustee and the builder share one code
/// path. Both are the ZIP 244 shielded signature hash.
pub fn sighash(pczt: &Pczt) -> Result<[u8; 32], Box<dyn Error>> {
    if *pczt.global().tx_version() != V6_TX_VERSION {
        return Err("only v6 (NU6.3, Ironwood) transactions are supported".into());
    }
    let signer = pczt::roles::signer::Signer::new(pczt.clone())
        .map_err(|e| format!("the PCZT does not describe a complete transaction yet: {e:?}"))?;
    Ok(signer.shielded_sighash())
}

// sealForge: `spends_to_sign`, `apply` and `serialize` removed; sealForge's
// signers find their spends and apply signatures in sealforge-chain.
