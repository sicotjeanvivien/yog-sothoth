use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2UpdateRewardFunderEvent};

#[async_trait]
pub trait MeteoraDammV2UpdateRewardFunderEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2UpdateRewardFunderEvent) -> RepositoryResult<()>;
}
