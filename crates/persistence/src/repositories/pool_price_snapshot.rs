//! Postgres implementation of [`PoolPriceSnapshotRepository`], over the
//! `pool_price_snapshot` view.
//!
//! [`PoolPriceSnapshotRepository`]: yog_core::domain::PoolPriceSnapshotRepository

mod rows;

use crate::repositories::helper::{convert_decimal_to_bigdecimal, map_sqlx_error};
use async_trait::async_trait;
use rows::PoolPriceSnapshotRow;
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{PoolPriceSnapshot, PoolPriceSnapshotRepository},
};

pub struct PgPoolPriceSnapshotRepository {
    pool: PgPool,
}

impl PgPoolPriceSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolPriceSnapshotRepository for PgPoolPriceSnapshotRepository {
    async fn latest(&self, min_tvl_usd: Decimal) -> RepositoryResult<Vec<PoolPriceSnapshot>> {
        let min_tvl_usd = convert_decimal_to_bigdecimal(min_tvl_usd, "min_tvl_usd")?;
        // sqlx cannot infer nullability through a view: `!` states what its INNER
        // joins and `IS NOT NULL` predicates guarantee.
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
            WHERE yog_amount_usd(reserve_a, decimals_a, price_a_usd)
                + yog_amount_usd(reserve_b, decimals_b, price_b_usd) >= $1
            "#,
            min_tvl_usd,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolPriceSnapshot::try_from).collect()
    }
}
