//! Postgres implementation of `TokenPriceRepository`.
//!
//! Backed by the `token_prices` hypertable (migration 004).
mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::TokenPriceRow;
use solana_pubkey::Pubkey;
use sqlx::{PgPool, QueryBuilder};
use yog_core::{
    RepositoryResult,
    domain::{TokenPrice, TokenPriceRepository},
};

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
        if prices.is_empty() {
            return Ok(());
        }

        // Variable-arity bulk insert: QueryBuilder is the right tool
        // here, the `query!` macros can't generate VALUES tuples at
        // a runtime-determined arity.
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

        builder.push(" ON CONFLICT (mint, fetched_at) DO NOTHING");

        builder
            .build()
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>> {
        let row = sqlx::query_as!(
            TokenPriceRow,
            r#"
            SELECT mint,
                  price_usd AS "price_usd!: rust_decimal::Decimal",
                  price_source, confidence, fetched_at
            FROM token_prices
            WHERE mint = $1
            ORDER BY fetched_at DESC
            LIMIT 1
            "#,
            mint.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TokenPrice::try_from).transpose()
    }
}
