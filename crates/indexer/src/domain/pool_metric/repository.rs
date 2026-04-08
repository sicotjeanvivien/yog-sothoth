use async_trait::async_trait;
use yog_core::CoreResult;
use crate::domain::PoolMetric;

#[async_trait]
pub(crate) trait PoolMetricRepository {
    /// Persist a pool metric snapshot.
    async fn insert(&self, metric: &PoolMetric) -> CoreResult<()>;

    /// Retrieve metrics for a pool over a time range, ordered by timestamp descending.
    async fn find_by_pool(
        &self,
        pool_address: &str,
        limit: i64,
    ) -> CoreResult<Vec<PoolMetric>>;
}