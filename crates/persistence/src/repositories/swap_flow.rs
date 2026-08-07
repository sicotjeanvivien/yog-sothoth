//! Postgres implementation of [`SwapFlowRepository`].
//!
//! Reads the `meteora_damm_v2_pool_hourly_flow` view (baseline §15), which
//! encapsulates the per-(pool, hour) directional USD valuation. This query
//! just windows and sums it per pool — a slim `SELECT` the sqlx macro still
//! verifies against the view's columns.
//!
//! [`SwapFlowRepository`]: yog_core::domain::SwapFlowRepository

mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::PoolSwapFlowRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{PoolSwapFlow, SwapFlowRepository},
};

/// Postgres-backed directional swap flow repository.
pub struct PgSwapFlowRepository {
    pool: PgPool,
}

impl PgSwapFlowRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SwapFlowRepository for PgSwapFlowRepository {
    async fn directional_volume_since(
        &self,
        since: DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolSwapFlow>> {
        // Postgres pushes the `bucket > $1` predicate down into the swap CA,
        // so this only touches recent buckets (no materialization).
        //
        // ⚠️ No COALESCE. It used to keep a direction with no priced volume at
        // 0 rather than NULL, which made "unknown" indistinguishable from a
        // real zero — and since the two directions were coalesced
        // INDEPENDENTLY, one unpriced side yielded `(0 − X)/X = −1.0` exactly:
        // a guaranteed maximum-magnitude Critical from `flow_imbalance` on a
        // possibly balanced pool (`.project` ticket 08).
        //
        // `bool_and` is the other half, and it is not redundant: `SUM` skips
        // NULLs on its own, so dropping the COALESCE alone would still publish
        // a **sub-total** for a partially valuable window, silently. Requiring
        // the whole window makes the sub-total unrepresentable — either every
        // bucket was valuable and the sum is a true total, or the caller gets
        // NULL and the detector skips the pool.
        let rows = sqlx::query_as!(
            PoolSwapFlowRow,
            r#"
            SELECT
                pool_address AS "pool_address!",
                CASE WHEN bool_and(valuation_complete)
                     THEN SUM(volume_a_to_b_usd) END AS "volume_a_to_b_usd?",
                CASE WHEN bool_and(valuation_complete)
                     THEN SUM(volume_b_to_a_usd) END AS "volume_b_to_a_usd?"
            FROM meteora_damm_v2_pool_hourly_flow
            WHERE bucket > $1
            GROUP BY pool_address
            "#,
            since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapFlow::try_from).collect()
    }
}
