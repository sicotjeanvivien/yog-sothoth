use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2LiquidityEvent},
};

/// Write side of liquidity events, the indexer's lens.
#[async_trait]
pub trait MeteoraDammV2LiquidityEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<InsertOutcome>;
}
