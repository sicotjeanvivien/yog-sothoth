use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryResult, domain::TokenActivity};

use crate::repositories::helper::{convert_bigdecimal_to_decimal, convert_string_to_pubkey};
use crate::repositories::token_metadata::rows::TokenMetadataRow;

/// The hour of the last ranking, beside one of its tokens and that token's
/// `token_metadata` row: the token columns are all `NULL` when the page is
/// empty, the metadata ones until yog-context resolves the mint.
pub(super) struct TokenActivityRow {
    pub(super) last_hour: Option<DateTime<Utc>>,
    pub(super) mint: Option<String>,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) pools: Option<i64>,
    pub(super) pool_hours: Option<i64>,
    pub(super) pool_hours_valued: Option<i64>,
    pub(super) symbol: Option<String>,
    pub(super) name: Option<String>,
    pub(super) decimals: Option<i16>,
    pub(super) logo_uri: Option<String>,
    pub(super) metadata_provider: Option<String>,
    pub(super) fetched_at: Option<DateTime<Utc>>,
    pub(super) last_refresh_at: Option<DateTime<Utc>>,
}

impl TokenActivityRow {
    /// `None` for the row of an empty page.
    pub(super) fn into_activity(self) -> RepositoryResult<Option<TokenActivity>> {
        let row = self;
        let (Some(mint), Some(volume_usd), Some(pools), Some(pool_hours), Some(pool_hours_valued)) = (
            row.mint,
            row.volume_usd,
            row.pools,
            row.pool_hours,
            row.pool_hours_valued,
        ) else {
            return Ok(None);
        };
        let metadata = match (
            row.decimals,
            row.metadata_provider,
            row.fetched_at,
            row.last_refresh_at,
        ) {
            (Some(decimals), Some(metadata_provider), Some(fetched_at), Some(last_refresh_at)) => {
                Some(
                    TokenMetadataRow {
                        mint: mint.clone(),
                        symbol: row.symbol,
                        name: row.name,
                        decimals,
                        logo_uri: row.logo_uri,
                        metadata_provider,
                        fetched_at,
                        last_refresh_at,
                    }
                    .try_into()?,
                )
            }
            _ => None,
        };
        Ok(Some(TokenActivity {
            mint: convert_string_to_pubkey(mint, "mint")?,
            metadata,
            volume_usd: convert_bigdecimal_to_decimal(volume_usd, "volume_usd")?,
            pools,
            pool_hours,
            pool_hours_valued,
        }))
    }
}
