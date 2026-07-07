//! Postgres implementation of [`LiquidityFlowRepository`].
//!
//! Reads the `meteora_damm_v2_pool_hourly_liquidity_flow` view (migration
//! 025), which encapsulates the per-(pool, hour) directional USD valuation
//! of liquidity events, joined with `pool_current_tvl` (migration 020) for
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
        // liquidity CA, so this only touches recent buckets. COALESCE keeps
        // a direction with no priced flow at 0 rather than NULL; `tvl_usd`
        // stays nullable on purpose (LEFT JOIN — a pool with no valued
        // current state must surface as unvaluable, not vanish, so the
        // detector can count what it skips).
        let rows = sqlx::query_as!(
            PoolLiquidityFlowRow,
            r#"
            SELECT
                f.pool_address                          AS "pool_address!",
                COALESCE(SUM(f.added_usd), 0::NUMERIC)  AS "added_usd!",
                COALESCE(SUM(f.removed_usd), 0::NUMERIC) AS "removed_usd!",
                t.tvl_usd                               AS "tvl_usd?"
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
