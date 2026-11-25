//! An item of `/api/protocols/{protocol}/tvl/by-token`: one token at the protocol's last reading.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::TokenTvl;

use super::token::EmbeddedTokenResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

/// ⚠️ A pool counts for both its tokens: the tokens add up to twice `total`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenTvlResponse {
    token: EmbeddedTokenResponse,
    /// The valued sides of its pools.
    #[schema(value_type = UnsignedDecimal)]
    tvl_usd: Decimal,
    /// `null` for a protocol without locks, or when none of its locked pools is valued.
    #[schema(value_type = Option<UnsignedDecimal>)]
    locked_usd: Option<Decimal>,
    /// Its pools, valued or not.
    #[schema(value_type = Count)]
    pools: i64,
}

impl From<TokenTvl> for TokenTvlResponse {
    fn from(t: TokenTvl) -> Self {
        Self {
            token: EmbeddedTokenResponse::from_sources(Some(t.mint), t.metadata),
            tvl_usd: t.tvl_usd,
            locked_usd: t.locked_usd,
            pools: t.pools,
        }
    }
}
