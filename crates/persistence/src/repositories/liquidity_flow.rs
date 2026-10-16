//! Postgres implementation of [`LiquidityFlowRepository`]: windows and sums the
//! `meteora_damm_v2_pool_hourly_liquidity_flow` view, joined with `pool_current_tvl`.
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

pub struct PgLiquidityFlowRepository {
    pool: PgPool,
}

impl PgLiquidityFlowRepository {
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
        // LEFT JOIN: a pool with no valued current state surfaces with a NULL `tvl_usd`, not vanishes.
        // ⚠️ No COALESCE and `bool_and`, as in the swap flow: a sub-total would under-estimate the drain.
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
