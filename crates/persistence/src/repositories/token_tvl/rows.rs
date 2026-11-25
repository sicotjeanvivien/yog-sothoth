use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryResult, domain::TokenTvl};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, convert_string_to_pubkey,
};
use crate::repositories::token_metadata::rows::TokenMetadataRow;

/// The reading's instant and totals beside one of its tokens and that token's `token_metadata`
/// row: the token columns are all `NULL` when the page is empty.
pub(super) struct TokenTvlRow {
    pub(super) at: Option<DateTime<Utc>>,
    pub(super) total_tvl_usd: Option<BigDecimal>,
    pub(super) total_locked_usd: Option<BigDecimal>,
    pub(super) mint: Option<String>,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) locked_usd: Option<BigDecimal>,
    pub(super) pools: Option<i64>,
    pub(super) symbol: Option<String>,
    pub(super) name: Option<String>,
    pub(super) decimals: Option<i16>,
    pub(super) logo_uri: Option<String>,
    pub(super) metadata_provider: Option<String>,
    pub(super) fetched_at: Option<DateTime<Utc>>,
    pub(super) last_refresh_at: Option<DateTime<Utc>>,
}

impl TokenTvlRow {
    /// `None` for the row of an empty page.
    pub(super) fn into_token(self) -> RepositoryResult<Option<TokenTvl>> {
        let row = self;
        let (Some(mint), Some(tvl_usd), Some(pools)) = (row.mint, row.tvl_usd, row.pools) else {
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
        Ok(Some(TokenTvl {
            mint: convert_string_to_pubkey(mint, "mint")?,
            metadata,
            tvl_usd: convert_bigdecimal_to_decimal(tvl_usd, "tvl_usd")?,
            locked_usd: convert_optional(
                row.locked_usd,
                "locked_usd",
                convert_bigdecimal_to_decimal,
            )?,
            pools,
        }))
    }
}
