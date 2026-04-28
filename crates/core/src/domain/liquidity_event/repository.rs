use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{domain::LiquidityEvent, RepositoryResult};

#[async_trait]
pub trait LiquidityEventRepository: Send + Sync {
    async fn insert(&self, event: &LiquidityEvent) -> RepositoryResult<()>;

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<LiquidityEvent>>;
}
