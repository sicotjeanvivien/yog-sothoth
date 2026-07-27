use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2WithdrawDeadLiquidityRewardEvent};

#[async_trait]
pub trait MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    ) -> RepositoryResult<()>;
}
