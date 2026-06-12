use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2SetPoolStatusEvent};

/// Write-side contract for DAMM v2 set-pool-status events.
///
/// Read-side methods are deliberately omitted until an API endpoint needs
/// them — adding them now would be dead code.
#[async_trait]
pub trait MeteoraDammV2SetPoolStatusEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2SetPoolStatusEvent) -> RepositoryResult<()>;
}
