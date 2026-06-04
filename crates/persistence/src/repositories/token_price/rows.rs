use crate::repositories::helper::convert_string_to_pubkey;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{PriceSource, TokenPrice},
};

/// Row shape for reading `token_prices`. Kept separate so the
/// fallible Pubkey + PriceSource conversions live in `TryFrom`
/// rather than scattered in the query function.
#[derive(sqlx::FromRow)]
pub(super) struct TokenPriceRow {
    pub(super) mint: String,
    pub(super) price_usd: Decimal,
    pub(super) price_source: String,
    pub(super) confidence: Option<f32>,
    pub(super) fetched_at: DateTime<Utc>,
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

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
