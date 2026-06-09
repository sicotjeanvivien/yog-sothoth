use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::MeteoraDammV2ClaimPositionFeeEvent};

#[async_trait]
pub trait MeteoraDammV2ClaimPositionFeeEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2ClaimPositionFeeEvent) -> RepositoryResult<()>;

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2ClaimPositionFeeEvent>>;
}
