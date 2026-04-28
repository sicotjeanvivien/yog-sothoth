use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{domain::ClaimPositionFeeEvent, RepositoryResult};

#[async_trait]
pub trait ClaimPositionFeeEventRepository: Send + Sync {
    async fn insert(&self, event: &ClaimPositionFeeEvent) -> RepositoryResult<()>;

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<ClaimPositionFeeEvent>>;
}
