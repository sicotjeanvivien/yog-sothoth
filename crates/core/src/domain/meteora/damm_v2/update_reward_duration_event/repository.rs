use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2UpdateRewardDurationEvent};

#[async_trait]
pub trait MeteoraDammV2UpdateRewardDurationEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2UpdateRewardDurationEvent) -> RepositoryResult<()>;
}
