//! Pool price snapshot repository trait.
//!
//! Read-only access to per-pool current price inputs (raw on-chain
//! `sqrt_price` + latest oracle prices). The concrete `Pg` implementation
//! (reading the `pool_price_snapshot` view) lives in `yog-persistence`.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::PoolPriceSnapshot};

/// Read contract for pool price snapshots.
#[async_trait]
pub trait PoolPriceSnapshotRepository: Send + Sync {
    /// The current snapshot of every comparable pool: mints resolved, at
    /// least one swap observed, and an oracle price known for both tokens.
    /// Pools missing any of those inputs are absent from the result —
    /// staleness filtering is left to the caller.
    async fn latest(&self) -> RepositoryResult<Vec<PoolPriceSnapshot>>;
}
