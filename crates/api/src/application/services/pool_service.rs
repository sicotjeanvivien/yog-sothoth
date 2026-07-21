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

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::{
    Page, PageDirection, PagePosition, PoolSort, RepositoryResult,
    domain::{
        Pool, PoolAnalytics, PoolAnalyticsRepository, PoolCatalog, PoolCurrentState,
        PoolCurrentStateLookup, PoolCursor, PoolHistoryBucket, PoolRankMetric, SignalFeed,
        SignalRecord, TokenMetadataLookup, TokenPriceLookup,
    },
};

use crate::application::{EnrichedPool, EnrichedToken};

/// Window of the pools-list signal indicator. Signals are append-only
/// conclusions with no "resolved" state, so "this pool has signals" is
/// *defined* by a lookback window — 24h, consistent with the rest of
/// the list's analytics (volume/fees 24h).
const RECENT_SIGNALS_WINDOW_HOURS: i64 = 24;

/// Per-pool cap on the signals returned with a list page. Bounds the
/// payload against a noisy pool; the hover list has no use for more.
const RECENT_SIGNALS_PER_POOL: i64 = 20;

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
/// A pool's latest observed state plus its derived spot price.
///
/// `spot_price_a_in_b` is the on-chain `last_sqrt_price` decoded to a human
/// price (units of token B per 1 token A) via
/// [`yog_core::amm::damm_v2::sqrt_price_to_price_a_in_b`]. `None` when there is
/// no `sqrt_price` yet, or the token decimals needed to rescale it are not
/// resolved — derived in the service so the HTTP layer only formats.
pub(crate) struct PoolCurrentStateView {
    pub(crate) state: PoolCurrentState,
    pub(crate) spot_price_a_in_b: Option<Decimal>,
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
    pub(crate) fee_bps: Option<Decimal>,
    pub(crate) limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------
pub(crate) struct PoolService {
    pool_repository: Arc<dyn PoolCatalog>,
    pool_current_state_repository: Arc<dyn PoolCurrentStateLookup>,
    pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
    token_metadata_repository: Arc<dyn TokenMetadataLookup>,
    token_price_repository: Arc<dyn TokenPriceLookup>,
    signal_feed: Arc<dyn SignalFeed>,
}

impl PoolService {
    pub(crate) fn new(
        pool_repository: Arc<dyn PoolCatalog>,
        pool_current_state_repository: Arc<dyn PoolCurrentStateLookup>,
        pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
        token_metadata_repository: Arc<dyn TokenMetadataLookup>,
        token_price_repository: Arc<dyn TokenPriceLookup>,
        signal_feed: Arc<dyn SignalFeed>,
    ) -> Self {
        Self {
            pool_repository,
            pool_current_state_repository,
            pool_analytics_repository,
            token_metadata_repository,
            token_price_repository,
            signal_feed,
        }
    }

    /// The recent signals of every address in `addresses`, in one
    /// batched query — see [`RECENT_SIGNALS_WINDOW_HOURS`] /
    /// [`RECENT_SIGNALS_PER_POOL`]. Shared by every listing path so
    /// the wire field means the same thing on all pool endpoints.
    async fn recent_signals_by_pool(
        &self,
        addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, Vec<SignalRecord>>> {
        let since = Utc::now() - Duration::hours(RECENT_SIGNALS_WINDOW_HOURS);
        self.signal_feed
            .recent_by_pools(addresses, since, RECENT_SIGNALS_PER_POOL)
            .await
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
                params.fee_bps,
                params.limit,
            )
            .await?;

        let addresses: Vec<solana_pubkey::Pubkey> =
            page.items.iter().map(|p| p.pool_address).collect();
        let mut analytics = self
            .pool_analytics_repository
            .batch_compute(&addresses)
            .await?;
        let mut recent_signals = self.recent_signals_by_pool(&addresses).await?;

        let mut items = Vec::with_capacity(page.items.len());
        for pool in page.items {
            let pool_analytics = analytics
                .remove(&pool.pool_address)
                .unwrap_or_else(PoolAnalytics::empty);
            let signals = recent_signals
                .remove(&pool.pool_address)
                .unwrap_or_default();
            items.push(self.enrich(pool, pool_analytics, signals).await?);
        }

        Ok(EnrichedPoolPage {
            items,
            next_cursor: page.next_cursor,
            prev_cursor: page.prev_cursor,
            is_first: page.is_first,
            is_last: page.is_last,
        })
    }

    /// The distinct base-fee tiers observed across all pools, ascending —
    /// powers the fee filter's option list (`GET /api/pools/fee-tiers`). A
    /// thin pass-through: no enrichment, just the repository's list.
    pub(crate) async fn list_fee_tiers(&self) -> RepositoryResult<Vec<Decimal>> {
        self.pool_repository.list_fee_tiers().await
    }

    /// Top-N pools ranked by `metric`, each enriched with token context and
    /// analytics — powers `GET /api/pools/top`.
    ///
    /// Choreography:
    ///   1. `top_pool_addresses` → the ranked addresses (read-time ranking,
    ///      highest first, pools with no metric value excluded).
    ///   2. `find_by_addresses` + `batch_compute` → the pool rows and their
    ///      analytics in one query each.
    ///   3. emit in **rank order**, re-imposing it over the unordered batch
    ///      reads. A ranked address with no pool row is skipped defensively.
    pub(crate) async fn top_pools(
        &self,
        metric: PoolRankMetric,
        limit: i64,
    ) -> RepositoryResult<Vec<EnrichedPool>> {
        let ranked = self
            .pool_analytics_repository
            .top_pool_addresses(metric, limit)
            .await?;
        if ranked.is_empty() {
            return Ok(Vec::new());
        }

        let pools = self.pool_repository.find_by_addresses(&ranked).await?;
        let mut analytics = self
            .pool_analytics_repository
            .batch_compute(&ranked)
            .await?;
        let mut recent_signals = self.recent_signals_by_pool(&ranked).await?;

        let mut by_address: std::collections::HashMap<solana_pubkey::Pubkey, Pool> =
            pools.into_iter().map(|p| (p.pool_address, p)).collect();

        let mut items = Vec::with_capacity(ranked.len());
        for address in &ranked {
            let Some(pool) = by_address.remove(address) else {
                continue;
            };
            let pool_analytics = analytics
                .remove(address)
                .unwrap_or_else(PoolAnalytics::empty);
            let signals = recent_signals.remove(address).unwrap_or_default();
            items.push(self.enrich(pool, pool_analytics, signals).await?);
        }

        Ok(items)
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
        let signals = self
            .recent_signals_by_pool(&[*address])
            .await?
            .remove(address)
            .unwrap_or_default();

        Ok(Some(self.enrich(pool, analytics, signals).await?))
    }

    pub(crate) async fn get_latest_state(
        &self,
        address: &str,
    ) -> RepositoryResult<Option<PoolCurrentStateView>> {
        let Some(state) = self
            .pool_current_state_repository
            .get_by_address(address)
            .await?
        else {
            return Ok(None);
        };

        // The spot price lives in `last_sqrt_price`, but rescaling it to human
        // units needs both tokens' decimals (resolved by yog-context) — absent
        // either input, the price is simply unknown, not faked.
        let spot_price_a_in_b = match state.last_sqrt_price {
            Some(sqrt_price) => {
                self.spot_price_a_in_b(&state.pool_address, sqrt_price)
                    .await?
            }
            None => None,
        };

        Ok(Some(PoolCurrentStateView {
            state,
            spot_price_a_in_b,
        }))
    }

    /// Decode a pool's spot price (token B per 1 token A, human units) from its
    /// `sqrt_price`, resolving the token decimals it needs. `None` when the
    /// pool's mints or their metadata are not yet resolved.
    async fn spot_price_a_in_b(
        &self,
        pool_address: &solana_pubkey::Pubkey,
        sqrt_price: u128,
    ) -> RepositoryResult<Option<Decimal>> {
        let Some(pool) = self.pool_repository.find_by_address(pool_address).await? else {
            return Ok(None);
        };
        let (Some(mint_a), Some(mint_b)) = (pool.token_a_mint, pool.token_b_mint) else {
            return Ok(None);
        };
        let (Some(md_a), Some(md_b)) = (
            self.token_metadata_repository.find_by_mint(&mint_a).await?,
            self.token_metadata_repository.find_by_mint(&mint_b).await?,
        ) else {
            return Ok(None);
        };

        Ok(yog_core::amm::damm_v2::sqrt_price_to_price_a_in_b(
            sqrt_price,
            md_a.decimals,
            md_b.decimals,
        ))
    }

    /// Hourly activity history (volume, fees, liquidity, claims — all USD) for
    /// a pool over the last `days` days. A thin pass-through to the analytics
    /// repository: no enrichment, the series is self-contained.
    pub(crate) async fn get_history(
        &self,
        address: &solana_pubkey::Pubkey,
        days: i32,
    ) -> RepositoryResult<Vec<PoolHistoryBucket>> {
        self.pool_analytics_repository.history(address, days).await
    }

    /// Compose a pool with both enriched token sides and its analytics.
    ///
    /// Sequential awaits: at single-request latency the four indexed
    /// lookups are cheap, and readability wins over micro-parallelism.
    async fn enrich(
        &self,
        pool: Pool,
        analytics: PoolAnalytics,
        recent_signals: Vec<SignalRecord>,
    ) -> RepositoryResult<EnrichedPool> {
        let token_a = self.enrich_side(pool.token_a_mint).await?;
        let token_b = self.enrich_side(pool.token_b_mint).await?;

        Ok(EnrichedPool {
            token_a,
            token_b,
            analytics,
            pool,
            recent_signals,
        })
    }

    /// Enrich one token side. See [`EnrichedToken::resolve`].
    async fn enrich_side(
        &self,
        mint: Option<solana_pubkey::Pubkey>,
    ) -> RepositoryResult<EnrichedToken> {
        EnrichedToken::resolve(
            mint,
            self.token_metadata_repository.as_ref(),
            self.token_price_repository.as_ref(),
        )
        .await
    }
}

#[cfg(test)]
#[path = "tests/pool_service_tests.rs"]
mod tests;
