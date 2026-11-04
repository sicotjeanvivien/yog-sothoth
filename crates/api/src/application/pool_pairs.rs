//! The token pair of the pool an item is on, read for a whole batch at once.

use std::collections::HashMap;
use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        LiquidityChangeEvent, LiveRecord, PoolCatalog, PoolChangeEvent, PoolCreationEvent,
        PositionChangeEvent, PositionFeeClaimEvent, ProtocolFeeClaimEvent, RewardChangeEvent,
        RewardClaimEvent, SignalRecord, TokenMetadata, TokenMetadataLookup,
    },
};

use crate::application::EnrichedToken;

/// An item on one pool.
pub(crate) trait OnPool {
    fn pool_address(&self) -> Pubkey;

    /// The mints the item carries itself, read in place of its pool's.
    /// ⚠️ A pool just created has none in the catalog until yog-context resolves it.
    fn own_mints(&self) -> Option<(Pubkey, Pubkey)> {
        None
    }
}

macro_rules! on_pool {
    ($($item:ty),+) => {
        $(impl OnPool for $item {
            fn pool_address(&self) -> Pubkey {
                <$item>::pool_address(self)
            }
        })+
    };
}

on_pool!(
    LiquidityChangeEvent,
    PositionChangeEvent,
    PositionFeeClaimEvent,
    ProtocolFeeClaimEvent,
    RewardClaimEvent,
    RewardChangeEvent,
    PoolChangeEvent
);

impl OnPool for PoolCreationEvent {
    fn pool_address(&self) -> Pubkey {
        PoolCreationEvent::pool_address(self)
    }

    fn own_mints(&self) -> Option<(Pubkey, Pubkey)> {
        Some(self.mints())
    }
}

impl OnPool for SignalRecord {
    fn pool_address(&self) -> Pubkey {
        self.signal.pool_address
    }
}

impl<E: OnPool> OnPool for LiveRecord<E> {
    fn pool_address(&self) -> Pubkey {
        self.event.pool_address()
    }

    fn own_mints(&self) -> Option<(Pubkey, Pubkey)> {
        self.event.own_mints()
    }
}

/// The two token sides of each pool of a batch.
pub(crate) type PairsByPool = HashMap<Pubkey, (EnrichedToken, EnrichedToken)>;

/// An item with its pool's two token sides, unresolved when the pool or its mints are unknown.
#[derive(Debug, Clone)]
pub(crate) struct Paired<I> {
    pub(crate) item: I,
    pub(crate) token_a: EnrichedToken,
    pub(crate) token_b: EnrichedToken,
}

impl<I: OnPool> Paired<I> {
    pub(crate) fn of(item: I, pairs: &PairsByPool) -> Self {
        let (token_a, token_b) = pairs
            .get(&item.pool_address())
            .cloned()
            .unwrap_or_else(|| (EnrichedToken::unresolved(), EnrichedToken::unresolved()));
        Self {
            item,
            token_a,
            token_b,
        }
    }
}

/// The pairs of the pools items are on, over the catalog and metadata lenses.
pub(crate) struct PoolPairs {
    pools: Arc<dyn PoolCatalog>,
    metadata: Arc<dyn TokenMetadataLookup>,
}

impl PoolPairs {
    pub(crate) fn new(pools: Arc<dyn PoolCatalog>, metadata: Arc<dyn TokenMetadataLookup>) -> Self {
        Self { pools, metadata }
    }

    /// The pairs of `items`' pools, in two reads whatever their number. A pool the catalog
    /// omits is absent from the map.
    pub(crate) async fn of<I: OnPool>(&self, items: &[I]) -> RepositoryResult<PairsByPool> {
        let mut mints: HashMap<Pubkey, (Option<Pubkey>, Option<Pubkey>)> = HashMap::new();
        let mut unknown = Vec::new();
        for item in items {
            match item.own_mints() {
                Some((a, b)) => {
                    mints.insert(item.pool_address(), (Some(a), Some(b)));
                }
                None => unknown.push(item.pool_address()),
            }
        }
        unknown.retain(|pool| !mints.contains_key(pool));
        unknown.sort_unstable();
        unknown.dedup();
        if !unknown.is_empty() {
            for pool in self.pools.find_by_addresses(&unknown).await? {
                mints.insert(pool.pool_address, (pool.token_a_mint, pool.token_b_mint));
            }
        }

        let mut wanted: Vec<Pubkey> = mints
            .values()
            .flat_map(|&(a, b)| [a, b])
            .flatten()
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        let metadata: HashMap<Pubkey, TokenMetadata> = if wanted.is_empty() {
            HashMap::new()
        } else {
            self.metadata
                .find_by_mints(&wanted)
                .await?
                .into_iter()
                .map(|m| (m.mint, m))
                .collect()
        };

        let side = |mint: Option<Pubkey>| EnrichedToken {
            mint,
            metadata: mint.and_then(|m| metadata.get(&m).cloned()),
        };
        Ok(mints
            .into_iter()
            .map(|(pool, (a, b))| (pool, (side(a), side(b))))
            .collect())
    }

    /// A page, each item with its pool's pair.
    pub(crate) async fn page<I: OnPool>(&self, page: Page<I>) -> RepositoryResult<Page<Paired<I>>> {
        let pairs = self.of(&page.items).await?;
        Ok(Page {
            items: page
                .items
                .into_iter()
                .map(|item| Paired::of(item, &pairs))
                .collect(),
            next_cursor: page.next_cursor,
            prev_cursor: page.prev_cursor,
            is_first: page.is_first,
            is_last: page.is_last,
        })
    }
}

#[cfg(test)]
#[path = "pool_pairs_tests.rs"]
mod tests;
