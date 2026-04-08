use crate::domain::LiquidityEvent;
use async_trait::async_trait;
use yog_core::CoreResult;

#[async_trait]
pub(crate) trait LiquidityEventRepository {
    /// Persist a liquidity event.
    async fn insert(&self, event: &LiquidityEvent) -> CoreResult<()>;

    /// Retrieve all liquidity events for a pool, ordered by timestamp descending.
    async fn find_by_pool(&self, pool_address: &str, limit: i64)
        -> CoreResult<Vec<LiquidityEvent>>;
}
