use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2WithdrawIneligibleRewardEvent};

#[async_trait]
pub trait MeteoraDammV2WithdrawIneligibleRewardEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2WithdrawIneligibleRewardEvent,
    ) -> RepositoryResult<()>;
}
