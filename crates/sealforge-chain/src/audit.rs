//! Independent escrow rescan for auditors (task 3.4).
//!
//! Uses only the escrow's published incoming viewing key (UIVK) and a public
//! lightwallet server: compact blocks are trial-decrypted to find payments to
//! the escrow, then each payment's full transaction is fetched to read its
//! memo. No coordinator data is used, so the result can be compared with what
//! the coordinator published.

use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Context};
use orchard::{
    keys::{IncomingViewingKey, PreparedIncomingViewingKey},
    note_encryption::{DomainVersion, IronwoodVersion, NoteEncryptionDomain, OrchardVersion},
};
use zcash_client_backend::{
    proto::service::{BlockId, BlockRange, TxFilter},
    scanning::{scan_block, ScanningKey, ScanningKeyOps, ScanningKeys, SpendIdentifiers},
};
use zcash_keys::keys::UnifiedIncomingViewingKey;
use zcash_note_encryption::try_note_decryption;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    memo::Memo,
};

use crate::{wallet::ReceivedPayment, ChainParams, Client};

type Key = ScanningKey<IncomingViewingKey, orchard::keys::FullViewingKey, u32>;

/// Every payment to the escrow in blocks `from..=to`, oldest first.
pub async fn scan_incoming(
    client: &mut Client,
    params: ChainParams,
    uivk: &str,
    from: u32,
    to: u32,
) -> anyhow::Result<Vec<ReceivedPayment>> {
    let uivk = UnifiedIncomingViewingKey::decode(&params, uivk)
        .map_err(|e| anyhow!("bad viewing key: {e}"))?;
    let ivk = uivk
        .orchard()
        .clone()
        .ok_or_else(|| anyhow!("viewing key has no Orchard/Ironwood component"))?;

    let orchard: HashMap<u8, Box<dyn ScanningKeyOps<_, u32, _> + Send + Sync>> =
        HashMap::from([(0u8, Box::new(Key::new(ivk.clone(), None, 0, None)) as _)]);
    let ironwood: HashMap<u8, Box<dyn ScanningKeyOps<_, u32, _> + Send + Sync>> =
        HashMap::from([(0u8, Box::new(Key::new(ivk.clone(), None, 0, None)) as _)]);
    let keys = ScanningKeys::new(HashMap::new(), orchard, ironwood);

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
    let mut prior = None;
    while let Some(block) = stream.message().await? {
        let scanned = scan_block(
            &params,
            block,
            &keys,
            &SpendIdentifiers::empty(),
            prior.as_ref(),
            |_| Ok::<_, std::convert::Infallible>(None),
        )
        .map_err(|e| anyhow!("scanning block: {e:?}"))?;
        for tx in scanned.transactions() {
            if !tx.orchard_outputs().is_empty() || !tx.ironwood_outputs().is_empty() {
                hits.insert(*tx.txid().as_ref(), u32::from(scanned.height()));
            }
        }
        prior = Some(scanned.to_block_metadata());
    }

    // 2. Full transactions: values, output indices and memos.
    let prepared = PreparedIncomingViewingKey::new(&ivk);
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
        let tx = Transaction::read(
            &raw.data[..],
            BranchId::for_height(&params, BlockHeight::from_u32(height)),
        )
        .context("parsing transaction")?;
        if let Some(bundle) = tx.ironwood_bundle() {
            decrypt::<IronwoodVersion>(bundle, &prepared, txid, height, &mut out);
        }
        if let Some(bundle) = tx.orchard_bundle() {
            decrypt::<OrchardVersion>(bundle, &prepared, txid, height, &mut out);
        }
    }
    out.sort_by_key(|p| (p.block_height, p.txid, p.output_index));
    Ok(out)
}

fn decrypt<V: DomainVersion>(
    bundle: &orchard::Bundle<orchard::bundle::Authorized, zcash_protocol::value::ZatBalance>,
    ivk: &PreparedIncomingViewingKey,
    txid: [u8; 32],
    height: u32,
    out: &mut Vec<ReceivedPayment>,
) {
    for (index, action) in bundle.actions().iter().enumerate() {
        let domain = NoteEncryptionDomain::<V>::for_action(action);
        if let Some((note, _, memo)) = try_note_decryption(&domain, ivk, action) {
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
