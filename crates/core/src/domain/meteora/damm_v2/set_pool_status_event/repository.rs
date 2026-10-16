use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2SetPoolStatusEvent},
};

/// Write-side contract for DAMM v2 set-pool-status events.
#[async_trait]
pub trait MeteoraDammV2SetPoolStatusEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2SetPoolStatusEvent,
    ) -> RepositoryResult<InsertOutcome>;
}
