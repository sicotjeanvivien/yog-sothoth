use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{PriceProvider, TokenPrice},
};

use crate::repositories::helper::convert_string_to_pubkey;

/// Row shape of `token_prices`.
#[derive(sqlx::FromRow)]
pub(in crate::repositories) struct TokenPriceRow {
    pub(in crate::repositories) mint: String,
    pub(in crate::repositories) price_usd: Decimal,
    pub(in crate::repositories) price_provider: String,
    pub(in crate::repositories) confidence: Option<f32>,
    pub(in crate::repositories) fetched_at: DateTime<Utc>,
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

/// Reverse of `PriceProvider::as_str`. ⚠️ The schema lets any string in: an
/// unknown value is an integrity error.
fn parse_price_provider(raw: &str) -> RepositoryResult<PriceProvider> {
    PriceProvider::from_str(raw)
        .map_err(|e| RepositoryError::Integrity(format!("invalid price_provider: {e}")))
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
