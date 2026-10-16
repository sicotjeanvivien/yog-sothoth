use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2ClosePositionEvent},
};

/// Write-side contract for DAMM v2 close-position events.
#[async_trait]
pub trait MeteoraDammV2ClosePositionEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2ClosePositionEvent,
    ) -> RepositoryResult<InsertOutcome>;
}
