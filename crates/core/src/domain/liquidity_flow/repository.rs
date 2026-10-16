//! Windowed liquidity flow joined with current TVL.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{RepositoryResult, domain::PoolLiquidityFlow};

/// Read contract for windowed liquidity flow with current TVL.
#[async_trait]
pub trait LiquidityFlowRepository: Send + Sync {
    /// Per-pool flow over the hourly buckets after `since`, the caller's own
    /// clock; pools without a liquidity event are absent.
    async fn liquidity_flow_since(
        &self,
        since: DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolLiquidityFlow>>;
}
