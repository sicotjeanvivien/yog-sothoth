use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2ClaimPositionFeeEvent};

#[async_trait]
pub trait MeteoraDammV2ClaimPositionFeeEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2ClaimPositionFeeEvent) -> RepositoryResult<()>;
}
