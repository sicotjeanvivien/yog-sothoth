use crate::{domain::PoolMetric, RepositoryResult};
use async_trait::async_trait;
use solana_pubkey::Pubkey;

#[async_trait]
pub trait PoolMetricRepository: Send + Sync {
    /// Persist a pool metric snapshot.
    async fn insert(&self, metric: &PoolMetric) -> RepositoryResult<()>;

    /// Retrieve metrics for a pool over a time range, ordered by timestamp descending.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<PoolMetric>>;
}
