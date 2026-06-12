use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2CreatePositionEvent};

/// Write-side contract for DAMM v2 create-position events.
///
/// Read-side methods (pagination, per-pool listing) are deliberately omitted
/// until an API endpoint needs them — adding them now would be dead code.
#[async_trait]
pub trait MeteoraDammV2CreatePositionEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2CreatePositionEvent) -> RepositoryResult<()>;
}
