use async_trait::async_trait;
use solana_pubkey::Pubkey;
use std::collections::HashMap;

use crate::{
    RepositoryResult,
    domain::{PoolAnalytics, PoolHistoryBucket},
};

/// The metric a top-N pool ranking is ordered by.
///
/// One variant today (`Volume24h`); kept as an enum because the `/api/pools/top`
/// endpoint contract exposes a `metric` parameter meant to grow (e.g. `Tvl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolRankMetric {
    /// Realized 24h volume in USD (trade-time valued), descending.
    Volume24h,
}

/// Read-only access to derived analytics over pools.
///
/// Implementations live in `yog-persistence`. The repository never
/// writes; it only joins RPC and context tables to produce metrics
/// at query time.
#[async_trait]
pub trait PoolAnalyticsRepository: Send + Sync {
    /// Compute TVL and 24h volume in USD for the given pools.
    ///
    /// Returns a map keyed by pool address. Pools requested but not
    /// present in the map should be treated as
    /// [`PoolAnalytics::empty`] by the caller; the implementation
    /// is free to omit them from the result.
    async fn batch_compute(
        &self,
        pool_addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, PoolAnalytics>>;

    /// Hourly activity history for a single pool over the last `days` days,
    /// one [`PoolHistoryBucket`] per hour that had any activity, ordered by
    /// bucket ascending (oldest first — chart-ready). Buckets with no activity
    /// in any of the four sources are simply absent (sparse series).
    async fn history(
        &self,
        pool_address: &Pubkey,
        days: i32,
    ) -> RepositoryResult<Vec<PoolHistoryBucket>>;

    /// Pool addresses ranked by `metric`, highest first, capped at `limit`.
    ///
    /// Read-time ranking over the priced analytics (no materialization): pools
    /// with a NULL/absent metric value (unpriceable, or no activity in the
    /// window) are excluded rather than sorted last. The returned order is the
    /// ranking — callers preserve it when enriching for display.
    async fn top_pool_addresses(
        &self,
        metric: PoolRankMetric,
        limit: i64,
    ) -> RepositoryResult<Vec<Pubkey>>;
}
