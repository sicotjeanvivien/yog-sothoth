use async_trait::async_trait;

use crate::{RepositoryResult, domain::MeteoraDammV2InitializePoolEvent};

/// Write-side contract for DAMM v2 initialize-pool events.
///
/// Read-side methods are deliberately omitted until an API endpoint needs
/// them — adding them now would be dead code.
#[async_trait]
pub trait MeteoraDammV2InitializePoolEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2InitializePoolEvent) -> RepositoryResult<()>;
}
