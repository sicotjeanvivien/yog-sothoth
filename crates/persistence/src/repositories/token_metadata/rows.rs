use std::str::FromStr;

use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{MetadataProvider, TokenMetadata},
};

use crate::repositories::helper::{convert_i16_to_u8, convert_string_to_pubkey};

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

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
