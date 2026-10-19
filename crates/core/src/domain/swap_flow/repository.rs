//! Directional swap volume, from the hourly swap aggregate valued in USD.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{RepositoryResult, domain::PoolSwapFlow};

/// Read contract for directional swap flow.
#[async_trait]
pub trait SwapFlowRepository: Send + Sync {
    /// Per-pool volume over the hourly buckets after `since`, the caller's own clock: the pools
    /// whose total reaches `min_volume_usd`, and every pool whose window is not wholly valuable.
    async fn directional_volume_since(
        &self,
        since: DateTime<Utc>,
        min_volume_usd: Decimal,
    ) -> RepositoryResult<Vec<PoolSwapFlow>>;
}
