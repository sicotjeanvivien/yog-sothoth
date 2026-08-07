//! Postgres implementation of [`LiquidityFlowRepository`].
//!
//! Reads the `meteora_damm_v2_pool_hourly_liquidity_flow` view (migration
//! 025), which encapsulates the per-(pool, hour) directional USD valuation
//! of liquidity events, joined with `pool_current_tvl` (baseline §15) for
//! the current-TVL side of the drain ratio. This query just windows, sums
//! and joins — a slim `SELECT` the sqlx macro still verifies against the
//! views' columns.
//!
//! [`LiquidityFlowRepository`]: yog_core::domain::LiquidityFlowRepository

mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::PoolLiquidityFlowRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{LiquidityFlowRepository, PoolLiquidityFlow},
};

/// Postgres-backed liquidity flow repository.
pub struct PgLiquidityFlowRepository {
    pool: PgPool,
}

impl PgLiquidityFlowRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LiquidityFlowRepository for PgLiquidityFlowRepository {
    async fn liquidity_flow_since(
        &self,
        since: DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolLiquidityFlow>> {
        // Postgres pushes the `bucket > $1` predicate down into the
        // liquidity CA, so this only touches recent buckets. `tvl_usd` stays
        // nullable on purpose (LEFT JOIN — a pool with no valued current
        // state must surface as unvaluable, not vanish, so the detector can
        // count what it skips).
        //
        // ⚠️ No COALESCE, and `bool_and` for the same reasons as the swap
        // flow — written identically on purpose. The view already propagates
        // NULL across both token legs of a direction, which is the behaviour
        // `.project` ticket 08 wants; the defect was here, one level up, where
        // `SUM` skipped the unvaluable buckets and the COALESCE dressed the
        // remainder as a total. That sub-total slipped past `tvl_drain`'s
        // `tvl_usd` guard whenever the window was only PARTLY unpriced, and it
        // under-estimated the drain — a missed signal, silently.
        let rows = sqlx::query_as!(
            PoolLiquidityFlowRow,
            r#"
            SELECT
                f.pool_address AS "pool_address!",
                CASE WHEN bool_and(f.valuation_complete)
                     THEN SUM(f.added_usd) END   AS "added_usd?",
                CASE WHEN bool_and(f.valuation_complete)
                     THEN SUM(f.removed_usd) END AS "removed_usd?",
                t.tvl_usd                        AS "tvl_usd?"
            FROM meteora_damm_v2_pool_hourly_liquidity_flow f
            LEFT JOIN pool_current_tvl t ON t.pool_address = f.pool_address
            WHERE f.bucket > $1
            GROUP BY f.pool_address, t.tvl_usd
            "#,
            since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolLiquidityFlow::try_from).collect()
    }
}
