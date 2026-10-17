use async_trait::async_trait;

use crate::RepositoryResult;
use crate::domain::{InsertOutcome, MeteoraDammV2SwapEvent};

/// Write side of swap events, the indexer's lens.
#[async_trait]
pub trait MeteoraDammV2SwapEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2SwapEvent) -> RepositoryResult<InsertOutcome>;
}
