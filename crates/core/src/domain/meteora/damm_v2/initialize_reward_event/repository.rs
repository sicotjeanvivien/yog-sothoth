use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2InitializeRewardEvent};

#[async_trait]
pub trait MeteoraDammV2InitializeRewardEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2InitializeRewardEvent) -> RepositoryResult<()>;
}
