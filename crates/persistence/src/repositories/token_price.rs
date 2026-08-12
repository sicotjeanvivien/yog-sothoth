//! Postgres implementation of `TokenPriceRepository`.
//!
//! Backed by the `token_prices` hypertable (baseline §7).
mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::TokenPriceRow;
use solana_pubkey::Pubkey;
use sqlx::{PgPool, QueryBuilder};
use yog_core::{
    RepositoryResult,
    domain::{TokenPrice, TokenPriceLookup, TokenPriceRepository},
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

        // The trait states the contract; this makes breaking it loud in dev.
        // Not a validation — filtering here would put the rule in `persistence`,
        // and it already lives in `TokenPrice::is_storable` and in the schema.
        // What this catches is a *new caller* that forgot to apply it, which in
        // production would not fail visibly: it would abort the whole batch and
        // leave a permanent hole in the price history.
        debug_assert!(
            prices.iter().all(TokenPrice::is_storable),
            "insert_batch received a price the column cannot hold — the caller \
             must filter on TokenPrice::is_storable first, or this batch aborts \
             and every other mint in it is lost for good"
        );

        // Variable-arity bulk insert: QueryBuilder is the right tool
        // here, the `query!` macros can't generate VALUES tuples at
        // a runtime-determined arity.
        let mut builder = QueryBuilder::new(
            "INSERT INTO token_prices (mint, price_usd, price_provider, confidence, fetched_at) ",
        );

        builder.push_values(prices, |mut row, price| {
            row.push_bind(price.mint.to_string())
                .push_bind(price.price_usd)
                .push_bind(price.price_provider.as_str())
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
}

#[async_trait]
impl TokenPriceLookup for PgTokenPriceRepository {
    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>> {
        let row = sqlx::query_as!(
            TokenPriceRow,
            r#"
            SELECT mint,
                  price_usd AS "price_usd!: rust_decimal::Decimal",
                  price_provider, confidence, fetched_at
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
