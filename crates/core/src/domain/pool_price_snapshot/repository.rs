//! Current price inputs per pool, from the chain and the oracle.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::PoolPriceSnapshot};

/// Read contract for pool price snapshots.
#[async_trait]
pub trait PoolPriceSnapshotRepository: Send + Sync {
    /// Every comparable pool: mints resolved, a swap observed, both tokens priced.
    async fn latest(&self) -> RepositoryResult<Vec<PoolPriceSnapshot>>;
}
