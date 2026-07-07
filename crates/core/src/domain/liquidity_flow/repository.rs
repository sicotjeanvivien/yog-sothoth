//! Liquidity flow repository trait.
//!
//! Read-only access to windowed liquidity flow (added/removed USD) joined
//! with current TVL. The concrete `Pg` implementation (reading the
//! `meteora_damm_v2_pool_hourly_liquidity_flow` view joined with
//! `pool_current_tvl`) lives in `yog-persistence`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{RepositoryResult, domain::PoolLiquidityFlow};

/// Read contract for windowed liquidity flow with current TVL.
#[async_trait]
pub trait LiquidityFlowRepository: Send + Sync {
    /// Per-pool USD liquidity flow, summed over every hourly bucket since
    /// `since` (exclusive), with the pool's current TVL. Pools with no
    /// liquidity event in the window are absent from the result — no
    /// movement, no drain to measure.
    ///
    /// `since` is the caller's own cutoff — typically its frozen tick clock
    /// minus its window — not the database clock, so evaluation stays
    /// deterministic and independent of when the query runs.
    async fn liquidity_flow_since(
        &self,
        since: DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolLiquidityFlow>>;
}
