use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2UpdatePoolFeesEvent},
};

/// Write-side contract for DAMM v2 update-pool-fees events.
#[async_trait]
pub trait MeteoraDammV2UpdatePoolFeesEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2UpdatePoolFeesEvent,
    ) -> RepositoryResult<InsertOutcome>;
}
