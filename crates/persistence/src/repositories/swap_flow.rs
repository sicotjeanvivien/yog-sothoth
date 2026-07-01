//! Postgres implementation of [`SwapFlowRepository`].
//!
//! Reads the `meteora_damm_v2_pool_hourly_flow` view (migration 023), which
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
        // so this only touches recent buckets (no materialization). COALESCE
        // keeps a direction that had no priced volume at 0 rather than NULL.
        let rows = sqlx::query_as!(
            PoolSwapFlowRow,
            r#"
            SELECT
                pool_address                                 AS "pool_address!",
                COALESCE(SUM(volume_a_to_b_usd), 0::NUMERIC) AS "volume_a_to_b_usd!",
                COALESCE(SUM(volume_b_to_a_usd), 0::NUMERIC) AS "volume_b_to_a_usd!"
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
