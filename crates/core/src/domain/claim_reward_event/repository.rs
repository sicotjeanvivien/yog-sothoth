use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{domain::ClaimRewardEvent, RepositoryResult};

#[async_trait]
pub trait ClaimRewardEventRepository: Send + Sync {
    async fn insert(&self, event: &ClaimRewardEvent) -> RepositoryResult<()>;

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<ClaimRewardEvent>>;
}
