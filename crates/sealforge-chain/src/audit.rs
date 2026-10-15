//! Independent escrow rescan for auditors (task 3.4).
//!
//! Uses only the escrow's published incoming viewing key (UIVK) and a public
//! lightwallet server. The scanning itself is `sealforge-scan`, which the
//! browser Audit page runs too (task 3.5); this module feeds it over gRPC.

use std::collections::BTreeMap;

use anyhow::anyhow;
use sealforge_scan::{EscrowScanner, ReceivedPayment};
use zcash_client_backend::proto::service::{BlockId, BlockRange, TxFilter};

use crate::{ChainParams, Client};

/// Every payment to the escrow in blocks `from..=to`, oldest first.
pub async fn scan_incoming(
    client: &mut Client,
    params: ChainParams,
    uivk: &str,
    from: u32,
    to: u32,
) -> anyhow::Result<Vec<ReceivedPayment>> {
    let scanner = EscrowScanner::new(params, uivk).map_err(|e| anyhow!(e))?;

    // 1. Trial-decrypt compact blocks: which transactions pay the escrow.
    let mut stream = client
        .get_block_range(BlockRange {
            start: Some(BlockId {
                height: u64::from(from),
                hash: vec![],
            }),
            end: Some(BlockId {
                height: u64::from(to),
                hash: vec![],
            }),
            pool_types: vec![],
        })
        .await?
        .into_inner();
    let mut hits: BTreeMap<[u8; 32], u32> = BTreeMap::new();
    while let Some(block) = stream.message().await? {
        for txid in scanner.scan_compact_block(&block) {
            hits.insert(txid, block.height as u32);
        }
    }

    // 2. Full transactions: values, output indices and memos.
    let mut out = Vec::new();
    for (txid, height) in hits {
        let raw = client
            .get_transaction(TxFilter {
                block: None,
                index: 0,
                hash: txid.to_vec(),
            })
            .await?
            .into_inner();
        out.extend(
            scanner
                .decrypt_transaction(&raw.data, height)
                .map_err(|e| anyhow!(e))?,
        );
    }
    out.sort_by_key(|p| (p.block_height, p.txid, p.output_index));
    Ok(out)
}
