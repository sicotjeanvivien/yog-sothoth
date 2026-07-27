use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2SplitPositionEvent};

#[async_trait]
pub trait MeteoraDammV2SplitPositionEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2SplitPositionEvent) -> RepositoryResult<()>;
}
