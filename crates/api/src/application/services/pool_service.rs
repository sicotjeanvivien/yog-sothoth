//! Pool listing and enrichment: paginate, batch the analytics, fetch each side's token
//! context, assemble `EnrichedPool`. Domain-only: errors surface as `RepositoryError`.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::{
    RepositoryResult,
    domain::{
        FeeTier, Pool, PoolAnalytics, PoolAnalyticsRepository, PoolCatalog, PoolCurrentState,
        PoolCurrentStateLookup, PoolHistoryBucket, PoolListQuery, PoolPage, PoolPropertiesLookup,
        PoolRankMetric, PoolSideValues, SignalFeed, SignalRecord, TokenMetadataLookup,
    },
};

use crate::application::WorkSlots;
use crate::application::cache::{SharedCache, TOP_POOLS};
use crate::application::work_slots::no_work_slot;
use crate::application::{EnrichedPool, EnrichedPoolDetail, EnrichedToken};

/// Window of the pools-list signal indicator: signals are never resolved, so "has signals"
/// is defined by a lookback, 24 h like the list's other analytics.
const RECENT_SIGNALS_WINDOW_HOURS: i64 = 24;

/// Per-pool cap on the signals returned with a list page, against a noisy pool.
const RECENT_SIGNALS_PER_POOL: i64 = 20;

/// A page of enriched pools, with the pagination metadata of the underlying `PoolPage`.
pub(crate) struct EnrichedPoolPage {
    pub(crate) items: Vec<EnrichedPool>,
    pub(crate) next_cursor: Option<yog_core::Cursor>,
    pub(crate) prev_cursor: Option<yog_core::Cursor>,
    pub(crate) is_first: bool,
    pub(crate) is_last: bool,
    /// The traversal's anchor and how many pools moved above it, passed through —
    /// see [`yog_core::domain::PoolPage`].
    pub(crate) as_of: Option<DateTime<Utc>>,
    pub(crate) touched_since: i64,
}
/// A pool's latest observed state, its spot price (token B per token A, `None`
/// without a `sqrt_price` or resolved decimals) and the USD value of each side.
pub(crate) struct PoolCurrentStateView {
    pub(crate) state: PoolCurrentState,
    pub(crate) spot_price_a_in_b: Option<Decimal>,
    pub(crate) side_values: PoolSideValues,
}

/// The most pools `/api/pools/top` ranks: the shared ranking is computed at this size and
/// cut to each request's `limit`, and the HTTP layer refuses a larger one.
pub(crate) const TOP_POOLS_MAX: i64 = 20;

pub(crate) struct PoolService {
    pool_repository: Arc<dyn PoolCatalog>,
    pool_current_state_repository: Arc<dyn PoolCurrentStateLookup>,
    pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
    token_metadata_repository: Arc<dyn TokenMetadataLookup>,
    signal_feed: Arc<dyn SignalFeed>,
    /// One per protocol with a satellite, paired with a pool by
    /// [`PoolPropertiesLookup::protocol`]: the service names none of them.
    pool_properties_lookups: Vec<Arc<dyn PoolPropertiesLookup>>,
    /// `/api/pools/top`, computed once per metric and [`TOP_POOLS`] at [`TOP_POOLS_MAX`], cut to
    /// each `limit`. ⚠️ Keyed on the metric alone: keyed on `limit`, a client cycling limits
    /// would start a ranking per value.
    top_pools_cache: SharedCache<PoolRankMetric, Vec<EnrichedPool>>,
    /// Taken by a ranking while it computes, never by a caller waiting for
    /// the cached one.
    work_slots: WorkSlots,
}

/// What a [`PoolService`] reads from, and the slots its computations share.
pub(crate) struct PoolServiceDeps {
    pub(crate) pool_repository: Arc<dyn PoolCatalog>,
    pub(crate) pool_current_state_repository: Arc<dyn PoolCurrentStateLookup>,
    pub(crate) pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
    pub(crate) token_metadata_repository: Arc<dyn TokenMetadataLookup>,
    pub(crate) signal_feed: Arc<dyn SignalFeed>,
    pub(crate) pool_properties_lookups: Vec<Arc<dyn PoolPropertiesLookup>>,
    pub(crate) work_slots: WorkSlots,
}

impl PoolService {
    pub(crate) fn new(deps: PoolServiceDeps) -> Self {
        Self {
            pool_repository: deps.pool_repository,
            pool_properties_lookups: deps.pool_properties_lookups,
            pool_current_state_repository: deps.pool_current_state_repository,
            pool_analytics_repository: deps.pool_analytics_repository,
            token_metadata_repository: deps.token_metadata_repository,
            signal_feed: deps.signal_feed,
            top_pools_cache: SharedCache::new(TOP_POOLS),
            work_slots: deps.work_slots,
        }
    }

    /// The recent signals of every address, in one query. Shared by every listing path, so
    /// the wire field means the same on all pool endpoints.
    async fn recent_signals_by_pool(
        &self,
        addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, Vec<SignalRecord>>> {
        let since = Utc::now() - Duration::hours(RECENT_SIGNALS_WINDOW_HOURS);
        self.signal_feed
            .recent_by_pools(addresses, since, RECENT_SIGNALS_PER_POOL)
            .await
    }

    /// Paginates pools and enriches each. A pool absent from the analytics falls back to
    /// `PoolAnalytics::empty()`; missing metadata or price leaves the token's field `None`.
    pub(crate) async fn list_pools(
        &self,
        query: PoolListQuery,
    ) -> RepositoryResult<EnrichedPoolPage> {
        let PoolPage {
            page,
            as_of,
            touched_since,
        } = self.pool_repository.find_paginated(query).await?;

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
            as_of,
            touched_since,
        })
    }

    /// The most common base-fee tiers, with pool counts (`GET /api/pools/fee-tiers`).
    pub(crate) async fn list_fee_tiers(&self) -> RepositoryResult<Vec<FeeTier>> {
        self.pool_repository.list_fee_tiers().await
    }

    /// The pools ranked by `metric`, enriched (`GET /api/pools/top`), in rank order over the
    /// unordered batch reads; a ranked address with no pool row is skipped.
    pub(crate) async fn top_pools(
        &self,
        metric: PoolRankMetric,
        limit: i64,
    ) -> RepositoryResult<Vec<EnrichedPool>> {
        let ranked = self
            .top_pools_cache
            .get_or_compute(metric, async {
                let _slot = self.work_slots.acquire().await.ok_or_else(no_work_slot)?;
                self.compute_top_pools(metric, TOP_POOLS_MAX).await
            })
            .await?;
        let keep = usize::try_from(limit).unwrap_or(0);
        Ok(ranked.into_iter().take(keep).collect())
    }

    /// The ranking itself. Only [`Self::top_pools`] calls it, through the cache.
    async fn compute_top_pools(
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

    /// The pool detail sheet: the enriched pool and its protocol-specific properties, `None`
    /// for a pool never observed. The pool's own `protocol` selects the lookup.
    pub(crate) async fn get_pool_detail(
        &self,
        address: &solana_pubkey::Pubkey,
    ) -> RepositoryResult<Option<EnrichedPoolDetail>> {
        let Some(pool) = self.pool_repository.find_by_address(address).await? else {
            return Ok(None);
        };
        let protocol = pool.protocol;

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

        let properties = match self
            .pool_properties_lookups
            .iter()
            .find(|lookup| lookup.protocol() == protocol)
        {
            Some(lookup) => lookup.find_by_pool(address).await?,
            None => None,
        };

        Ok(Some(EnrichedPoolDetail {
            pool: self.enrich(pool, analytics, signals).await?,
            properties,
            evaluated_at: Utc::now(),
        }))
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

        // Rescaling needs both tokens' decimals: absent either, the price is unknown, not faked.
        let spot_price_a_in_b = match state.last_sqrt_price {
            Some(sqrt_price) => {
                self.spot_price_a_in_b(&state.pool_address, sqrt_price)
                    .await?
            }
            None => None,
        };
        let side_values = self
            .pool_current_state_repository
            .side_values_usd(&state.pool_address)
            .await?;

        Ok(Some(PoolCurrentStateView {
            state,
            spot_price_a_in_b,
            side_values,
        }))
    }

    /// A pool's spot price, token B per token A in human units, from its `sqrt_price`.
    /// `None` until its mints and their metadata are resolved.
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

    /// A pool's hourly activity in USD over the last `days` days.
    pub(crate) async fn get_history(
        &self,
        address: &solana_pubkey::Pubkey,
        days: i32,
    ) -> RepositoryResult<Vec<PoolHistoryBucket>> {
        self.pool_analytics_repository.history(address, days).await
    }

    /// A pool with both enriched token sides and its analytics, awaited in sequence.
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
        EnrichedToken::resolve(mint, self.token_metadata_repository.as_ref()).await
    }
}

#[cfg(test)]
#[path = "tests/pool_service_tests.rs"]
mod tests;
