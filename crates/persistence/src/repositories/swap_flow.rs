//! Postgres implementation of [`SwapFlowRepository`]: windows and sums the
//! `meteora_damm_v2_pool_hourly_flow` view (baseline §15) per pool.
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

pub struct PgSwapFlowRepository {
    pool: PgPool,
}

impl PgSwapFlowRepository {
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
        // ⚠️ No COALESCE: one unpriced side read as 0 gives `flow_imbalance` a −1.0. And `bool_and`
        // too: `SUM` skips NULLs, so a partly priced window would publish a sub-total as a total.
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
