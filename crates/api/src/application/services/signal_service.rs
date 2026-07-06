//! Application service for the signal feed.
//!
//! Orchestrates pagination of the signals emitted by the yog-signals daemon
//! detectors, and composes each signal with the token context of its
//! pool (same application-level enrichment as `PoolService`: the
//! signal only stores the pool address; the pair is resolved through
//! the catalog / metadata / price lenses). Pure domain: no axum, no
//! DTOs, no HTTP concerns. The handler is responsible for cursor wire
//! encoding/decoding and DTO mapping.

use std::collections::HashMap;
use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{
        Pool, PoolCatalog, Severity, SignalCursor, SignalFeed, SignalRecord, TokenMetadataLookup,
        TokenPriceLookup,
    },
    tools::Page,
};

use crate::application::{EnrichedSignal, EnrichedToken};

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Input to [`SignalService::list_signals`].
pub(crate) struct SignalListParams {
    pub severity: Option<Severity>,
    pub cursor: Option<SignalCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for signal feed queries.
///
/// Depends on the feed lens ([`SignalFeed`]) for the signals themselves
/// — the engine's write/dedup contract never reaches the api process —
/// plus the three read lenses needed to embed the pool's token pair.
pub(crate) struct SignalService {
    repo: Arc<dyn SignalFeed>,
    pool_catalog: Arc<dyn PoolCatalog>,
    token_metadata_repository: Arc<dyn TokenMetadataLookup>,
    token_price_repository: Arc<dyn TokenPriceLookup>,
}

impl SignalService {
    pub(crate) fn new(
        repo: Arc<dyn SignalFeed>,
        pool_catalog: Arc<dyn PoolCatalog>,
        token_metadata_repository: Arc<dyn TokenMetadataLookup>,
        token_price_repository: Arc<dyn TokenPriceLookup>,
    ) -> Self {
        Self {
            repo,
            pool_catalog,
            token_metadata_repository,
            token_price_repository,
        }
    }

    /// Paginate the signal feed, optionally filtered to one severity,
    /// each signal enriched with its pool's token pair.
    ///
    /// Each distinct pool of the page is resolved once (one batch
    /// lookup, then per-side metadata/price), however many signals
    /// point at it. A pool the catalog doesn't know — or knows without
    /// resolved mints — yields unresolved sides, never an error: the
    /// signal is the payload, the pair is context.
    pub(crate) async fn list_signals(
        &self,
        params: SignalListParams,
    ) -> Result<Page<EnrichedSignal>, RepositoryError> {
        let page = self
            .repo
            .list(
                params.severity,
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

        let sides = self.resolve_pools(&items).await?;

        let items = items
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
            .collect();

        Ok(Page {
            items,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        })
    }

    /// Enrich a single signal — the SSE path, one record at a time as
    /// the broadcast delivers them.
    pub(crate) async fn enrich_one(
        &self,
        record: SignalRecord,
    ) -> RepositoryResult<EnrichedSignal> {
        let pool = self
            .pool_catalog
            .find_by_address(&record.signal.pool_address)
            .await?;
        let (token_a, token_b) = self.enrich_sides(pool.as_ref()).await?;
        Ok(EnrichedSignal {
            record,
            token_a,
            token_b,
        })
    }

    /// Resolve the token sides of every distinct pool referenced by
    /// `records`: one `find_by_addresses` batch, then per-side
    /// resolution. Addresses the catalog omits are absent from the map
    /// — the caller falls back to unresolved sides.
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

    /// Both sides of one pool. `None` (pool unknown to the catalog)
    /// yields two unresolved sides.
    async fn enrich_sides(
        &self,
        pool: Option<&Pool>,
    ) -> RepositoryResult<(EnrichedToken, EnrichedToken)> {
        let (mint_a, mint_b) = match pool {
            Some(pool) => (pool.token_a_mint, pool.token_b_mint),
            None => (None, None),
        };
        Ok((
            EnrichedToken::resolve(
                mint_a,
                self.token_metadata_repository.as_ref(),
                self.token_price_repository.as_ref(),
            )
            .await?,
            EnrichedToken::resolve(
                mint_b,
                self.token_metadata_repository.as_ref(),
                self.token_price_repository.as_ref(),
            )
            .await?,
        ))
    }
}

#[cfg(test)]
#[path = "tests/signal_service_tests.rs"]
mod tests;
