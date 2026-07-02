//! Postgres implementation of [`PoolPriceSnapshotRepository`].
//!
//! Reads the `pool_price_snapshot` view (migration 024), which encapsulates
//! the join of a pool's current on-chain state with its tokens' decimals and
//! latest oracle prices. The query is a slim `SELECT` the sqlx macro still
//! verifies against the view's columns.
//!
//! [`PoolPriceSnapshotRepository`]: yog_core::domain::PoolPriceSnapshotRepository

mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::PoolPriceSnapshotRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{PoolPriceSnapshot, PoolPriceSnapshotRepository},
};

/// Postgres-backed pool price snapshot repository.
pub struct PgPoolPriceSnapshotRepository {
    pool: PgPool,
}

impl PgPoolPriceSnapshotRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolPriceSnapshotRepository for PgPoolPriceSnapshotRepository {
    async fn latest(&self) -> RepositoryResult<Vec<PoolPriceSnapshot>> {
        // Every column is forced non-null by the view's INNER joins and its
        // `IS NOT NULL` predicates; the `!` overrides tell sqlx so, since it
        // cannot infer nullability through a view.
        let rows = sqlx::query_as!(
            PoolPriceSnapshotRow,
            r#"
            SELECT
                pool_address       AS "pool_address!",
                protocol           AS "protocol!",
                last_sqrt_price    AS "last_sqrt_price!",
                last_swap_at       AS "last_swap_at!",
                decimals_a         AS "decimals_a!",
                decimals_b         AS "decimals_b!",
                price_a_usd        AS "price_a_usd!",
                price_a_fetched_at AS "price_a_fetched_at!",
                price_b_usd        AS "price_b_usd!",
                price_b_fetched_at AS "price_b_fetched_at!"
            FROM pool_price_snapshot
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolPriceSnapshot::try_from).collect()
    }
}
