use crate::domain::SwapEvent;
use async_trait::async_trait;
use yog_core::CoreResult;

#[async_trait]
pub(crate) trait SwapEventRepository {
    /// Persist a swap event.
    async fn insert(&self, event: &SwapEvent) -> CoreResult<()>;

    /// Retrieve all swap events for a pool, ordered by timestamp descending.
    async fn find_by_pool(&self, pool_address: &str, limit: i64) -> CoreResult<Vec<SwapEvent>>;
}
