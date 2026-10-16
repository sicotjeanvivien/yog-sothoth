use async_trait::async_trait;
use solana_pubkey::Pubkey;
use std::collections::HashMap;

use crate::{
    RepositoryResult,
    domain::{PoolAnalytics, PoolHistoryBucket},
};

/// The metric a top-N pool ranking is ordered by: a flow or a stock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolRankMetric {
    /// Realized 24h volume in USD (trade-time valued), descending.
    Volume24h,
    /// Current total value locked in USD, descending.
    Tvl,
}

/// Read-only analytics over pools, computed at query time.
#[async_trait]
pub trait PoolAnalyticsRepository: Send + Sync {
    /// The analytics of the given pools; an absent pool reads as [`PoolAnalytics::empty`].
    async fn batch_compute(
        &self,
        pool_addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, PoolAnalytics>>;

    /// One [`PoolHistoryBucket`] per active hour of the last `days` days, oldest
    /// first; an hour without activity is absent.
    async fn history(
        &self,
        pool_address: &Pubkey,
        days: i32,
    ) -> RepositoryResult<Vec<PoolHistoryBucket>>;

    /// Pool addresses ranked by `metric`, highest first, capped at `limit`; a pool
    /// without a value is excluded. The order is the ranking: callers keep it.
    async fn top_pool_addresses(
        &self,
        metric: PoolRankMetric,
        limit: i64,
    ) -> RepositoryResult<Vec<Pubkey>>;
}
