use std::str::FromStr;

use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{MetadataProvider, TokenEntry, TokenMetadata, TokenPrice},
};

use crate::repositories::helper::{convert_i16_to_u8, convert_string_to_pubkey};
use crate::repositories::token_price::rows::TokenPriceRow;

/// Row shape of `token_metadata`.
#[derive(sqlx::FromRow)]
pub(in crate::repositories) struct TokenMetadataRow {
    pub(in crate::repositories) mint: String,
    pub(in crate::repositories) symbol: Option<String>,
    pub(in crate::repositories) name: Option<String>,
    pub(in crate::repositories) decimals: i16,
    pub(in crate::repositories) logo_uri: Option<String>,
    pub(in crate::repositories) metadata_provider: String,
    pub(in crate::repositories) fetched_at: DateTime<Utc>,
    pub(in crate::repositories) last_refresh_at: DateTime<Utc>,
}

impl TryFrom<TokenMetadataRow> for TokenMetadata {
    type Error = RepositoryError;

    fn try_from(row: TokenMetadataRow) -> Result<Self, Self::Error> {
        Ok(TokenMetadata {
            mint: convert_string_to_pubkey(row.mint, "mint")?,
            symbol: row.symbol,
            name: row.name,
            decimals: convert_i16_to_u8(row.decimals, "decimals")?,
            logo_uri: row.logo_uri,
            metadata_provider: parse_metadata_provider(&row.metadata_provider)?,
            fetched_at: row.fetched_at,
            last_refresh_at: row.last_refresh_at,
        })
    }
}

fn parse_metadata_provider(raw: &str) -> RepositoryResult<MetadataProvider> {
    MetadataProvider::from_str(raw)
        .map_err(|e| RepositoryError::Integrity(format!("invalid price_provider: {e}")))
}

/// A token's metadata and its latest price, the price columns `None` before its first.
pub(super) struct TokenEntryRow {
    pub(super) mint: String,
    pub(super) symbol: Option<String>,
    pub(super) name: Option<String>,
    pub(super) decimals: i16,
    pub(super) logo_uri: Option<String>,
    pub(super) metadata_provider: String,
    pub(super) fetched_at: DateTime<Utc>,
    pub(super) last_refresh_at: DateTime<Utc>,
    pub(super) price_usd: Option<rust_decimal::Decimal>,
    pub(super) price_provider: Option<String>,
    pub(super) confidence: Option<f32>,
    pub(super) price_fetched_at: Option<DateTime<Utc>>,
}

impl TryFrom<TokenEntryRow> for TokenEntry {
    type Error = RepositoryError;

    fn try_from(row: TokenEntryRow) -> Result<Self, Self::Error> {
        let price = match (row.price_usd, row.price_provider, row.price_fetched_at) {
            (Some(price_usd), Some(price_provider), Some(fetched_at)) => {
                Some(TokenPrice::try_from(TokenPriceRow {
                    mint: row.mint.clone(),
                    price_usd,
                    price_provider,
                    confidence: row.confidence,
                    fetched_at,
                })?)
            }
            _ => None,
        };
        let metadata = TokenMetadata::try_from(TokenMetadataRow {
            mint: row.mint,
            symbol: row.symbol,
            name: row.name,
            decimals: row.decimals,
            logo_uri: row.logo_uri,
            metadata_provider: row.metadata_provider,
            fetched_at: row.fetched_at,
            last_refresh_at: row.last_refresh_at,
        })?;
        Ok(TokenEntry { metadata, price })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
