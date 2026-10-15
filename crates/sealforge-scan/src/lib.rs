//! Find and decrypt payments to an escrow with only its incoming viewing key
//! (task 3.4 natively, task 3.5 in the browser).
//!
//! No I/O, so the same code runs in `sealforge-audit` (blocks from tonic) and
//! on the Audit page (blocks from gRPC-web, compiled to WebAssembly):
//! 1. [`EscrowScanner::scan_compact_block`]: trial-decrypt a compact block's
//!    Orchard and Ironwood actions; returns the txids that pay the escrow.
//! 2. [`EscrowScanner::decrypt_transaction`]: decrypt those transactions in
//!    full for each payment's value, output index and memo.

pub mod params;
#[cfg(feature = "wasm")]
mod wasm;

pub use params::ChainParams;

use orchard::{
    keys::{IncomingViewingKey, PreparedIncomingViewingKey},
    note_encryption::{
        CompactAction, DomainVersion, IronwoodVersion, NoteEncryptionDomain, OrchardVersion,
    },
};
use zcash_client_backend::proto::compact_formats::{CompactBlock, CompactOrchardAction};
use zcash_keys::keys::UnifiedIncomingViewingKey;
use zcash_note_encryption::{try_compact_note_decryption, try_note_decryption};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    memo::Memo,
    value::ZatBalance,
};

/// A payment received by the escrow, as the scanner and auditor see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedPayment {
    /// Transaction id, internal byte order.
    pub txid: [u8; 32],
    pub output_index: u32,
    pub block_height: u32,
    pub amount_zat: u64,
    /// Text memo, if the payment carried one.
    pub memo: Option<String>,
}

pub struct EscrowScanner {
    params: ChainParams,
    ivk: PreparedIncomingViewingKey,
}

impl EscrowScanner {
    /// A scanner for the escrow behind a Unified incoming viewing key.
    pub fn new(params: ChainParams, uivk: &str) -> Result<Self, String> {
        let uivk = UnifiedIncomingViewingKey::decode(&params, uivk)
            .map_err(|e| format!("bad viewing key: {e}"))?;
        let ivk: &IncomingViewingKey = uivk
            .orchard()
            .as_ref()
            .ok_or("viewing key has no Orchard/Ironwood component")?;
        Ok(Self {
            params,
            ivk: PreparedIncomingViewingKey::new(ivk),
        })
    }

    pub fn params(&self) -> ChainParams {
        self.params
    }

    /// Txids (internal byte order) of the block's transactions that pay the escrow.
    pub fn scan_compact_block(&self, block: &CompactBlock) -> Vec<[u8; 32]> {
        let mut hits = Vec::new();
        for tx in &block.vtx {
            let pays = self.any_compact::<OrchardVersion>(&tx.actions)
                || self.any_compact::<IronwoodVersion>(&tx.ironwood_actions);
            if let (true, Ok(txid)) = (pays, <[u8; 32]>::try_from(tx.txid.as_slice())) {
                hits.push(txid);
            }
        }
        hits
    }

    fn any_compact<V: DomainVersion>(&self, actions: &[CompactOrchardAction]) -> bool {
        actions.iter().any(|a| {
            CompactAction::try_from(a).is_ok_and(|action| {
                let domain = NoteEncryptionDomain::<V>::for_compact_action(&action);
                try_compact_note_decryption(&domain, &self.ivk, &action).is_some()
            })
        })
    }

    /// Every payment to the escrow in a raw transaction mined at `height`.
    pub fn decrypt_transaction(
        &self,
        raw: &[u8],
        height: u32,
    ) -> Result<Vec<ReceivedPayment>, String> {
        let tx = Transaction::read(
            raw,
            BranchId::for_height(&self.params, BlockHeight::from_u32(height)),
        )
        .map_err(|e| format!("parsing transaction: {e}"))?;
        let txid: [u8; 32] = *tx.txid().as_ref();
        let mut out = Vec::new();
        if let Some(bundle) = tx.ironwood_bundle() {
            self.decrypt::<IronwoodVersion>(bundle, txid, height, &mut out);
        }
        if let Some(bundle) = tx.orchard_bundle() {
            self.decrypt::<OrchardVersion>(bundle, txid, height, &mut out);
        }
        Ok(out)
    }

    fn decrypt<V: DomainVersion>(
        &self,
        bundle: &orchard::Bundle<orchard::bundle::Authorized, ZatBalance>,
        txid: [u8; 32],
        height: u32,
        out: &mut Vec<ReceivedPayment>,
    ) {
        for (index, action) in bundle.actions().iter().enumerate() {
            let domain = NoteEncryptionDomain::<V>::for_action(action);
            if let Some((note, _, memo)) = try_note_decryption(&domain, &self.ivk, action) {
                let memo = match Memo::from_bytes(&memo) {
                    Ok(Memo::Text(text)) => Some(text.to_string()),
                    _ => None,
                };
                out.push(ReceivedPayment {
                    txid,
                    output_index: index as u32,
                    block_height: height,
                    amount_zat: note.value().inner(),
                    memo,
                });
            }
        }
    }
}
