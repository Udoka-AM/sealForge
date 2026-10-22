//! Browser bindings (task 4.7). The page runs these in a Web Worker: it
//! fetches compact blocks and tree states over gRPC-web, passes the raw
//! protobuf bytes in, and broadcasts the transactions that come out.

use prost::Message;
use wasm_bindgen::prelude::*;
use zcash_client_backend::proto::{compact_formats::CompactBlock, service::TreeState};

use crate::{chain_params, Keys, PaymentRequest, ScanError, Wallet, WalletState};

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// A new 24-word recovery phrase from 32 bytes of `crypto.getRandomValues`.
#[wasm_bindgen]
pub fn new_phrase(entropy: &[u8]) -> Result<String, JsError> {
    let entropy: [u8; 32] = entropy
        .try_into()
        .map_err(|_| err("need 32 bytes of entropy"))?;
    crate::phrase_from_entropy(entropy).map_err(err)
}

#[wasm_bindgen]
pub fn phrase_is_valid(phrase: &str) -> bool {
    crate::phrase_is_valid(phrase)
}

/// Build the proving key ahead of the first payment (a few seconds).
#[wasm_bindgen]
pub fn prepare_prover() {
    crate::prepare_prover();
}

#[wasm_bindgen]
pub struct WalletHandle(Wallet);

#[wasm_bindgen]
impl WalletHandle {
    /// A new wallet born at `birthday`; `tree_state` is the lightwalletd
    /// `TreeState` protobuf for block `birthday - 1`.
    pub fn create(
        network: &str,
        phrase: &str,
        birthday: u32,
        tree_state: &[u8],
    ) -> Result<WalletHandle, JsError> {
        let keys = Keys::from_phrase(chain_params(network).map_err(err)?, phrase).map_err(err)?;
        let tree = TreeState::decode(tree_state).map_err(err)?;
        Wallet::new(keys, network, birthday, &tree)
            .map(WalletHandle)
            .map_err(err)
    }

    /// Reopen a saved wallet (state from [`WalletHandle::state_json`]).
    pub fn open(network: &str, phrase: &str, state_json: &str) -> Result<WalletHandle, JsError> {
        let keys = Keys::from_phrase(chain_params(network).map_err(err)?, phrase).map_err(err)?;
        let state: WalletState = serde_json::from_str(state_json).map_err(err)?;
        if state.network != network {
            return Err(err(format!("this wallet is for {}", state.network)));
        }
        Wallet::from_state(keys, &state)
            .map(WalletHandle)
            .map_err(err)
    }

    pub fn state_json(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.0.to_state()).map_err(err)
    }

    /// A `CompactBlock` protobuf → JSON of what changed. Errors start with
    /// `reorg:` when the chain changed (call `reset` and sync again), or
    /// `out_of_order:` / `tree_mismatch:`.
    pub fn scan_block(&mut self, block: &[u8]) -> Result<String, JsError> {
        let block = CompactBlock::decode(block).map_err(err)?;
        match self.0.scan_block(&block) {
            Ok(outcome) => serde_json::to_string(&outcome).map_err(err),
            Err(ScanError::Reorg) => Err(err("reorg: the chain changed under the wallet")),
            Err(e @ ScanError::OutOfOrder { .. }) => Err(err(format!("out_of_order: {e}"))),
            Err(e @ ScanError::TreeMismatch(_)) => Err(err(format!("tree_mismatch: {e}"))),
            Err(e) => Err(err(e)),
        }
    }

    pub fn reset(&mut self) -> Result<(), JsError> {
        self.0.reset().map_err(err)
    }

    pub fn scanned_height(&self) -> u32 {
        self.0.scanned_height()
    }

    pub fn birthday(&self) -> u32 {
        self.0.birthday()
    }

    pub fn balance(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.0.balance()).map_err(err)
    }

    pub fn notes(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.0.notes().collect::<Vec<_>>()).map_err(err)
    }

    pub fn sent(&self) -> Result<String, JsError> {
        serde_json::to_string(self.0.sent()).map_err(err)
    }

    pub fn funding_address(&self) -> String {
        self.0.funding_address()
    }

    /// `{"index": n, "address": "u…"}`: an address used for nothing else.
    pub fn fresh_address(&mut self) -> Result<String, JsError> {
        let (index, address) = self.0.fresh_address();
        serde_json::to_string(&serde_json::json!({ "index": index, "address": address }))
            .map_err(err)
    }

    pub fn address(&self, index: u32) -> String {
        self.0.address(index)
    }

    /// The fee for paying `amount_zat` now; errors if the balance doesn't cover it.
    pub fn quote(&self, amount_zat: u64) -> Result<u64, JsError> {
        self.0.quote(amount_zat).map_err(err)
    }

    /// `{"to", "amount_zat", "memo"?, "label"?}` → `{"txid", "raw" (hex), "fee_zat", "expiry_height"}`.
    pub fn pay(&mut self, request_json: &str) -> Result<String, JsError> {
        let request: PaymentRequest = serde_json::from_str(request_json).map_err(err)?;
        let built = self.0.pay(&request).map_err(err)?;
        serde_json::to_string(&built).map_err(err)
    }

    /// The server refused a payment: release its notes.
    pub fn forget_payment(&mut self, txid: &str) {
        self.0.forget_payment(txid);
    }
}
