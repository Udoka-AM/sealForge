//! In-memory compact block cache for `zcash_client_backend::sync::run`.
//! Blocks are only held between download and scan, so memory stays small.

use std::{collections::BTreeMap, sync::Mutex};

use zcash_client_backend::{
    data_api::{
        chain::{error::Error as ChainError, BlockCache, BlockSource},
        scanning::ScanRange,
    },
    proto::compact_formats::CompactBlock,
};
use zcash_protocol::consensus::BlockHeight;

#[derive(Debug, Default)]
pub struct MemBlockCache(Mutex<BTreeMap<BlockHeight, CompactBlock>>);

#[derive(Debug)]
pub struct CacheError(String);

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CacheError {}

impl BlockSource for MemBlockCache {
    type Error = CacheError;

    fn with_blocks<F, WalletErrT>(
        &self,
        from_height: Option<BlockHeight>,
        limit: Option<usize>,
        mut with_block: F,
    ) -> Result<(), ChainError<WalletErrT, Self::Error>>
    where
        F: FnMut(CompactBlock) -> Result<(), ChainError<WalletErrT, Self::Error>>,
    {
        let blocks: Vec<CompactBlock> = {
            let map = self.0.lock().expect("cache lock");
            map.range(from_height.unwrap_or(BlockHeight::from_u32(0))..)
                .take(limit.unwrap_or(usize::MAX))
                .map(|(_, b)| b.clone())
                .collect()
        };
        for block in blocks {
            with_block(block)?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl BlockCache for MemBlockCache {
    fn get_tip_height(
        &self,
        range: Option<&ScanRange>,
    ) -> Result<Option<BlockHeight>, Self::Error> {
        let map = self.0.lock().expect("cache lock");
        Ok(match range {
            Some(r) => map
                .range(r.block_range().clone())
                .next_back()
                .map(|(h, _)| *h),
            None => map.keys().next_back().copied(),
        })
    }

    async fn read(&self, range: &ScanRange) -> Result<Vec<CompactBlock>, Self::Error> {
        let map = self.0.lock().expect("cache lock");
        Ok(map
            .range(range.block_range().clone())
            .map(|(_, b)| b.clone())
            .collect())
    }

    async fn insert(&self, compact_blocks: Vec<CompactBlock>) -> Result<(), Self::Error> {
        let mut map = self.0.lock().expect("cache lock");
        for block in compact_blocks {
            map.insert(block.height(), block);
        }
        Ok(())
    }

    async fn delete(&self, range: ScanRange) -> Result<(), Self::Error> {
        let mut map = self.0.lock().expect("cache lock");
        let heights: Vec<BlockHeight> = map
            .range(range.block_range().clone())
            .map(|(h, _)| *h)
            .collect();
        for h in heights {
            map.remove(&h);
        }
        Ok(())
    }
}
