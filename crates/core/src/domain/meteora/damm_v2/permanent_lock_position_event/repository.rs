use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2PermanentLockPositionEvent},
};

/// Write-side contract for DAMM v2 permanent-lock-position events.
#[async_trait]
pub trait MeteoraDammV2PermanentLockPositionEventRepository: Send + Sync {
    async fn insert(
        &self,
        event: &MeteoraDammV2PermanentLockPositionEvent,
    ) -> RepositoryResult<InsertOutcome>;
}
