//! Current price inputs per pool, from the chain and the oracle.

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::{RepositoryResult, domain::PoolPriceSnapshot};

/// Read contract for pool price snapshots.
#[async_trait]
pub trait PoolPriceSnapshotRepository: Send + Sync {
    /// Every comparable pool (mints resolved, a swap observed, both tokens priced) whose TVL at
    /// the last known prices, whatever their age, reaches `min_tvl_usd`.
    async fn latest(&self, min_tvl_usd: Decimal) -> RepositoryResult<Vec<PoolPriceSnapshot>>;
}
