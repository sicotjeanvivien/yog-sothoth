use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2FundRewardEvent},
};

#[async_trait]
pub trait MeteoraDammV2FundRewardEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2FundRewardEvent)
    -> RepositoryResult<InsertOutcome>;
}
