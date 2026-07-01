//! Swap flow repository trait.
//!
//! Read-only access to directional swap volume, derived from the hourly
//! swap continuous aggregate valued in USD. The concrete `Pg`
//! implementation (reading the `meteora_damm_v2_pool_hourly_flow` view)
//! lives in `yog-persistence`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{RepositoryResult, domain::PoolSwapFlow};

/// Read contract for directional swap flow.
#[async_trait]
pub trait SwapFlowRepository: Send + Sync {
    /// Per-pool directional USD swap volume, summed over every hourly
    /// bucket since `since` (exclusive). Pools with no priced swap activity
    /// in the window are absent from the result.
    ///
    /// `since` is the caller's own cutoff — typically its frozen tick clock
    /// minus its window — not the database clock, so evaluation stays
    /// deterministic and independent of when the query runs.
    async fn directional_volume_since(
        &self,
        since: DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolSwapFlow>>;
}
