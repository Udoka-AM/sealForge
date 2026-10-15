//! Browser bindings (task 3.5). JS fetches compact blocks and raw transactions
//! over gRPC-web from a lightwallet server the auditor chooses; the scanning
//! and decryption happen here, with only the published viewing key.

use prost::Message;
use wasm_bindgen::prelude::*;
use zcash_client_backend::proto::compact_formats::CompactBlock;

use crate::{ChainParams, EscrowScanner};

#[wasm_bindgen]
pub struct Scanner(EscrowScanner);

#[wasm_bindgen]
impl Scanner {
    /// `network`: "mainnet", "testnet" or "regtest".
    #[wasm_bindgen(constructor)]
    pub fn new(network: &str, uivk: &str) -> Result<Scanner, JsError> {
        let params = match network {
            "mainnet" => ChainParams::Mainnet,
            "testnet" => ChainParams::Testnet,
            "regtest" => ChainParams::regtest(),
            other => return Err(JsError::new(&format!("unknown network {other}"))),
        };
        EscrowScanner::new(params, uivk)
            .map(Scanner)
            .map_err(|e| JsError::new(&e))
    }

    /// A `CompactBlock` protobuf message → JSON list of txids (hex, internal
    /// byte order) that pay the escrow.
    pub fn scan_block(&self, block: &[u8]) -> Result<String, JsError> {
        let block = CompactBlock::decode(block).map_err(|e| JsError::new(&e.to_string()))?;
        let txids: Vec<String> = self
            .0
            .scan_compact_block(&block)
            .iter()
            .map(hex::encode)
            .collect();
        Ok(serde_json::to_string(&txids)?)
    }

    /// The height of a `CompactBlock` protobuf message.
    pub fn block_height(block: &[u8]) -> Result<u32, JsError> {
        let block = CompactBlock::decode(block).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(block.height as u32)
    }

    /// A raw transaction mined at `height` → JSON payments in the shape the
    /// auditor's `audit_with_payments` takes.
    pub fn decrypt(&self, raw: &[u8], height: u32) -> Result<String, JsError> {
        let payments = self
            .0
            .decrypt_transaction(raw, height)
            .map_err(|e| JsError::new(&e))?;
        let json: Vec<serde_json::Value> = payments
            .iter()
            .map(|p| {
                serde_json::json!({
                    "txid": hex::encode(p.txid),
                    "output_index": p.output_index,
                    "block_height": p.block_height,
                    "amount_zat": p.amount_zat,
                    "memo": p.memo,
                })
            })
            .collect();
        Ok(serde_json::to_string(&json)?)
    }
}
