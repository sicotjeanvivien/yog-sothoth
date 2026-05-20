//! Postgres implementation of `TokenPriceRepository`.
//!
//! Backed by the `token_prices` hypertable (migration 004).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::{PgPool, QueryBuilder};

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{PriceSource, TokenPrice, TokenPriceRepository},
};

use crate::repository_utils::{convert_string_to_pubkey, map_sqlx_error};

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
        // The (mint, fetched_at DESC) index from migration 004 makes
        // this a fast lookup: it picks the most recent fetch for the
        // given mint.
        let row = sqlx::query_as::<_, TokenPriceRow>(
            r#"
            SELECT mint, price_usd, price_source, confidence, fetched_at
            FROM token_prices
            WHERE mint = $1
            ORDER BY fetched_at DESC
            LIMIT 1
            "#,
        )
        .bind(mint.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TokenPrice::try_from).transpose()
    }
}

/// Row shape for reading `token_prices`. Kept separate so the
/// fallible Pubkey + PriceSource conversions live in `TryFrom`
/// rather than scattered in the query function.
#[derive(sqlx::FromRow)]
struct TokenPriceRow {
    mint: String,
    price_usd: Decimal,
    price_source: String,
    confidence: Option<f32>,
    fetched_at: DateTime<Utc>,
}

impl TryFrom<TokenPriceRow> for TokenPrice {
    type Error = RepositoryError;

    fn try_from(row: TokenPriceRow) -> Result<Self, Self::Error> {
        let price_source = parse_price_source(&row.price_source)?;

        Ok(TokenPrice {
            mint: convert_string_to_pubkey(row.mint, "mint")?,
            price_usd: row.price_usd,
            price_source,
            confidence: row.confidence,
            fetched_at: row.fetched_at,
        })
    }
}

/// Reverse of `PriceSource::as_str`. A value the domain does not
/// know about is a data integrity issue — the schema lets any
/// string in, only the writer side guards against that.
fn parse_price_source(raw: &str) -> RepositoryResult<PriceSource> {
    match raw {
        "jupiter" => Ok(PriceSource::Jupiter),
        "helius" => Ok(PriceSource::Helius),
        "fallback" => Ok(PriceSource::Fallback),
        other => Err(RepositoryError::Integrity(format!(
            "invalid price_source: {other}"
        ))),
    }
}
