use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{PriceProvider, TokenPrice},
};

use crate::repositories::helper::convert_string_to_pubkey;

/// Row shape for reading `token_prices`. Kept separate so the
/// fallible Pubkey + PriceProvider conversions live in `TryFrom`
/// rather than scattered in the query function.
#[derive(sqlx::FromRow)]
pub(super) struct TokenPriceRow {
    pub(super) mint: String,
    pub(super) price_usd: Decimal,
    pub(super) price_provider: String,
    pub(super) confidence: Option<f32>,
    pub(super) fetched_at: DateTime<Utc>,
}

impl TryFrom<TokenPriceRow> for TokenPrice {
    type Error = RepositoryError;

    fn try_from(row: TokenPriceRow) -> Result<Self, Self::Error> {
        Ok(TokenPrice {
            mint: convert_string_to_pubkey(row.mint, "mint")?,
            price_usd: row.price_usd,
            price_provider: parse_price_provider(&row.price_provider)?,
            confidence: row.confidence,
            fetched_at: row.fetched_at,
        })
    }
}

/// Reverse of `PriceProvider::as_str`. A value the domain does not
/// know about is a data integrity issue — the schema lets any
/// string in, only the writer side guards against that.
fn parse_price_provider(raw: &str) -> RepositoryResult<PriceProvider> {
    PriceProvider::from_str(raw)
        .map_err(|e| RepositoryError::Integrity(format!("invalid price_provider: {e}")))
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
