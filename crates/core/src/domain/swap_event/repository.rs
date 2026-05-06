use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::SwapEvent};

/// Persistence contract for swap events.
#[async_trait]
pub trait SwapEventRepository: Send + Sync {
    async fn insert(&self, event: &SwapEvent) -> RepositoryResult<()>;

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<SwapEvent>>;
}
