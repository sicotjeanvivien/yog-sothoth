//! The signal feed: pagination of the detectors' signals, each composed with its pool's token
//! pair through the catalog, metadata and price lenses. Pure domain.

use std::collections::HashMap;
use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    Page, PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{
        Pool, PoolCatalog, Severity, SignalCursor, SignalFeed, SignalRecord, TokenMetadataLookup,
    },
};

use crate::application::{EnrichedSignal, EnrichedToken};

/// Input to [`SignalService::list_signals`].
pub(crate) struct SignalListParams {
    pub severity: Option<Severity>,
    pub pool: Option<Pubkey>,
    pub cursor: Option<SignalCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

/// Signal feed queries, over the [`SignalFeed`] lens and the two lenses of the token pair.
pub(crate) struct SignalService {
    repo: Arc<dyn SignalFeed>,
    pool_catalog: Arc<dyn PoolCatalog>,
    token_metadata_repository: Arc<dyn TokenMetadataLookup>,
}

impl SignalService {
    pub(crate) fn new(
        repo: Arc<dyn SignalFeed>,
        pool_catalog: Arc<dyn PoolCatalog>,
        token_metadata_repository: Arc<dyn TokenMetadataLookup>,
    ) -> Self {
        Self {
            repo,
            pool_catalog,
            token_metadata_repository,
        }
    }

    /// Paginates the feed, each signal enriched with its pool's pair. An unknown or unresolved
    /// pool yields unresolved sides, never an error: the signal is the payload.
    pub(crate) async fn list_signals(
        &self,
        params: SignalListParams,
    ) -> Result<Page<EnrichedSignal>, RepositoryError> {
        let page = self
            .repo
            .list(
                params.severity,
                params.pool,
                params.cursor,
                params.direction,
                params.position,
                params.limit,
            )
            .await?;

        let Page {
            items,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        } = page;

        let items = self.enrich_batch(items).await?;

        Ok(Page {
            items,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        })
    }

    /// Enriches a batch of signals, each distinct pool resolved once. ⚠️ The poller calls it
    /// once per tick, before broadcasting, so a signal's cost does not grow with open streams.
    pub(crate) async fn enrich_batch(
        &self,
        records: Vec<SignalRecord>,
    ) -> RepositoryResult<Vec<EnrichedSignal>> {
        let sides = self.resolve_pools(&records).await?;

        Ok(records
            .into_iter()
            .map(|record| {
                let (token_a, token_b) = sides
                    .get(&record.signal.pool_address)
                    .cloned()
                    .unwrap_or_else(|| (EnrichedToken::unresolved(), EnrichedToken::unresolved()));
                EnrichedSignal {
                    record,
                    token_a,
                    token_b,
                }
            })
            .collect())
    }

    /// The token sides of every distinct pool of `records`, in one batch. An address the
    /// catalog omits is absent from the map.
    async fn resolve_pools(
        &self,
        records: &[SignalRecord],
    ) -> RepositoryResult<HashMap<Pubkey, (EnrichedToken, EnrichedToken)>> {
        let mut addresses: Vec<Pubkey> = records.iter().map(|r| r.signal.pool_address).collect();
        addresses.sort_unstable();
        addresses.dedup();
        if addresses.is_empty() {
            return Ok(HashMap::new());
        }

        let pools: HashMap<Pubkey, Pool> = self
            .pool_catalog
            .find_by_addresses(&addresses)
            .await?
            .into_iter()
            .map(|pool| (pool.pool_address, pool))
            .collect();

        let mut sides = HashMap::with_capacity(pools.len());
        for (address, pool) in &pools {
            sides.insert(*address, self.enrich_sides(Some(pool)).await?);
        }
        Ok(sides)
    }

    /// Both sides of one pool; an unknown pool yields two unresolved sides.
    async fn enrich_sides(
        &self,
        pool: Option<&Pool>,
    ) -> RepositoryResult<(EnrichedToken, EnrichedToken)> {
        let (mint_a, mint_b) = match pool {
            Some(pool) => (pool.token_a_mint, pool.token_b_mint),
            None => (None, None),
        };
        let metadata = self.token_metadata_repository.as_ref();
        Ok((
            EnrichedToken::resolve(mint_a, metadata).await?,
            EnrichedToken::resolve(mint_b, metadata).await?,
        ))
    }
}

#[cfg(test)]
#[path = "tests/signal_service_tests.rs"]
mod tests;
