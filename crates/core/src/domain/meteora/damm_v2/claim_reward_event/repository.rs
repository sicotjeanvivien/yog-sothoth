use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2ClaimRewardEvent};

#[async_trait]
pub trait MeteoraDammV2ClaimRewardEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2ClaimRewardEvent) -> RepositoryResult<()>;
}
