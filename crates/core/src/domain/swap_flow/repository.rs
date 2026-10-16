//! Directional swap volume, from the hourly swap aggregate valued in USD.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{RepositoryResult, domain::PoolSwapFlow};

/// Read contract for directional swap flow.
#[async_trait]
pub trait SwapFlowRepository: Send + Sync {
    /// Per-pool volume over the hourly buckets after `since`, the caller's own
    /// clock; pools without priced swaps are absent.
    async fn directional_volume_since(
        &self,
        since: DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolSwapFlow>>;
}
