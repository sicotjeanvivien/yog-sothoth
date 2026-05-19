//! Postgres implementation of `TokenPriceRepository`.
//!
//! Backed by the `token_prices` hypertable (migration 004).

use async_trait::async_trait;
use sqlx::{PgPool, QueryBuilder};

use yog_core::{
    RepositoryResult,
    domain::{TokenPrice, TokenPriceRepository},
};

use crate::repository_utils::map_sqlx_error;

/// Postgres-backed token price repository.
#[derive(Clone)]
pub struct PgTokenPriceRepository {
    pool: PgPool,
}

impl PgTokenPriceRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenPriceRepository for PgTokenPriceRepository {
    async fn insert_batch(&self, prices: &[TokenPrice]) -> RepositoryResult<()> {
        // Nothing to insert — skip the round-trip entirely.
        if prices.is_empty() {
            return Ok(());
        }

        // A single multi-row INSERT built with QueryBuilder, so one
        // tick of the price worker is one DB round-trip regardless of
        // how many mints were priced.
        //
        // - `mint`      : Pubkey -> TEXT via to_string().
        // - `price_usd` : rust_decimal::Decimal binds directly to
        //   NUMERIC (sqlx `rust_decimal` feature) — no convert helper.
        let mut builder = QueryBuilder::new(
            "INSERT INTO token_prices (mint, price_usd, price_source, confidence, fetched_at) ",
        );

        builder.push_values(prices, |mut row, price| {
            row.push_bind(price.mint.to_string())
                .push_bind(price.price_usd)
                .push_bind(price.price_source.as_str())
                .push_bind(price.confidence)
                .push_bind(price.fetched_at);
        });

        // token_prices is append-only and keyed by (mint, fetched_at).
        // A collision would only happen on an exact-timestamp replay;
        // ignore it rather than fail the whole batch.
        builder.push(" ON CONFLICT (mint, fetched_at) DO NOTHING");

        builder
            .build()
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }
}
