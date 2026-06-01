//! Application service orchestrating pool listing and enrichment.
//!
//! Owns the multi-repository choreography that used to live in the
//! HTTP handler: paginate pools, batch-compute their analytics, fetch
//! each side's token metadata and latest price, and assemble the
//! `EnrichedPool` aggregates.
//!
//! Domain-only boundary: this service depends on `yog-core` repository
//! traits and domain types exclusively. It knows nothing of axum,
//! `ApiError`, cursors-as-strings, or wire DTOs. Errors surface as
//! `RepositoryError` (re-exported through `RepositoryResult`); the HTTP
//! layer maps them to `ApiError`.

use std::sync::Arc;

use yog_core::{
    Page, PageDirection, PagePosition, PoolSort, RepositoryResult,
    domain::{
        Pool, PoolAnalytics, PoolAnalyticsRepository, PoolCurrentState, PoolCurrentStateRepository,
        PoolCursor, PoolRepository, TokenMetadataRepository, TokenPriceRepository,
    },
};

use crate::application::{EnrichedPool, EnrichedToken};

// ^ if you keep PoolListParams in its own file; otherwise define it here.

/// A page of enriched pools, preserving the pagination metadata from
/// the underlying `Page<Pool>`.
pub(crate) struct EnrichedPoolPage {
    pub(crate) items: Vec<EnrichedPool>,
    pub(crate) next_cursor: Option<yog_core::Cursor>,
    pub(crate) prev_cursor: Option<yog_core::Cursor>,
    pub(crate) is_first: bool,
    pub(crate) is_last: bool,
}
// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// All fields are domain types — the HTTP layer is responsible for
/// parsing query params, decoding the cursor, converting wire enums,
/// and normalizing the search term before constructing this.
pub(crate) struct PoolListParams {
    pub(crate) cursor: Option<PoolCursor>,
    pub(crate) direction: PageDirection,
    pub(crate) position: Option<PagePosition>,
    pub(crate) sort: PoolSort,
    pub(crate) search: Option<String>,
    pub(crate) limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------
pub(crate) struct PoolService {
    pool_repository: Arc<dyn PoolRepository>,
    pool_current_state_repository: Arc<dyn PoolCurrentStateRepository>,
    pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
    token_metadata_repository: Arc<dyn TokenMetadataRepository>,
    token_price_repository: Arc<dyn TokenPriceRepository>,
}

impl PoolService {
    pub(crate) fn new(
        pool_repository: Arc<dyn PoolRepository>,
        pool_current_state_repository: Arc<dyn PoolCurrentStateRepository>,
        pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
        token_metadata_repository: Arc<dyn TokenMetadataRepository>,
        token_price_repository: Arc<dyn TokenPriceRepository>,
    ) -> Self {
        Self {
            pool_repository,
            pool_current_state_repository,
            pool_analytics_repository,
            token_metadata_repository,
            token_price_repository,
        }
    }

    /// Paginate pools and enrich each with token context and analytics.
    ///
    /// Choreography:
    ///   1. `find_paginated` → a `Page<Pool>` with navigation metadata.
    ///   2. `batch_compute` → analytics for the whole page in one query.
    ///   3. per pool, both sides: metadata + latest price lookups.
    ///
    /// Pools absent from the analytics map fall back to
    /// `PoolAnalytics::empty()`. Missing metadata/price are tolerated
    /// (the enriched token carries `None`).
    pub(crate) async fn list_pools(
        &self,
        params: PoolListParams,
    ) -> RepositoryResult<EnrichedPoolPage> {
        let page: Page<Pool> = self
            .pool_repository
            .find_paginated(
                params.cursor,
                params.direction,
                params.position,
                params.sort,
                params.search,
                params.limit,
            )
            .await?;

        let addresses: Vec<solana_pubkey::Pubkey> =
            page.items.iter().map(|p| p.pool_address).collect();
        let mut analytics = self
            .pool_analytics_repository
            .batch_compute(&addresses)
            .await?;

        let mut items = Vec::with_capacity(page.items.len());
        for pool in page.items {
            let pool_analytics = analytics
                .remove(&pool.pool_address)
                .unwrap_or_else(PoolAnalytics::empty);
            items.push(self.enrich(pool, pool_analytics).await?);
        }

        Ok(EnrichedPoolPage {
            items,
            next_cursor: page.next_cursor,
            prev_cursor: page.prev_cursor,
            is_first: page.is_first,
            is_last: page.is_last,
        })
    }

    /// Fetch a single pool by address and enrich it. Returns `None` if
    /// the pool has never been observed.
    pub(crate) async fn get_pool(
        &self,
        address: &solana_pubkey::Pubkey,
    ) -> RepositoryResult<Option<EnrichedPool>> {
        let Some(pool) = self.pool_repository.find_by_address(address).await? else {
            return Ok(None);
        };

        let mut analytics_map = self
            .pool_analytics_repository
            .batch_compute(&[*address])
            .await?;
        let analytics = analytics_map
            .remove(address)
            .unwrap_or_else(PoolAnalytics::empty);

        Ok(Some(self.enrich(pool, analytics).await?))
    }

    pub(crate) async fn get_latest_state(
        &self,
        address: &str,
    ) -> RepositoryResult<Option<PoolCurrentState>> {
        self.pool_current_state_repository
            .get_by_address(address)
            .await
    }

    /// Compose a pool with both enriched token sides and its analytics.
    ///
    /// Sequential awaits: at single-request latency the four indexed
    /// lookups are cheap, and readability wins over micro-parallelism.
    async fn enrich(&self, pool: Pool, analytics: PoolAnalytics) -> RepositoryResult<EnrichedPool> {
        let token_a_meta = self
            .token_metadata_repository
            .find_by_mint(&pool.token_a_mint)
            .await?;
        let token_a_price = self
            .token_price_repository
            .find_latest_by_mint(&pool.token_a_mint)
            .await?;
        let token_b_meta = self
            .token_metadata_repository
            .find_by_mint(&pool.token_b_mint)
            .await?;
        let token_b_price = self
            .token_price_repository
            .find_latest_by_mint(&pool.token_b_mint)
            .await?;

        Ok(EnrichedPool {
            token_a: EnrichedToken {
                mint: pool.token_a_mint,
                metadata: token_a_meta,
                price: token_a_price,
            },
            token_b: EnrichedToken {
                mint: pool.token_b_mint,
                metadata: token_b_meta,
                price: token_b_price,
            },
            analytics,
            pool,
        })
    }
}

#[cfg(test)]
#[path = "tests/pool_service_tests.rs"]
mod tests;
