use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2ClaimProtocolFeeEvent},
};

#[async_trait]
pub trait MeteoraDammV2ClaimProtocolFeeEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2ClaimProtocolFeeEvent,
    ) -> RepositoryResult<InsertOutcome>;
}
