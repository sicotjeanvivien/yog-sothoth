use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::TokenTvlTotal;

use crate::http::dto::schema::UnsignedDecimal;

/// The protocol's own reading, not the tokens summed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct TokenTvlPageTotalResponse {
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<rust_decimal::Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    locked_usd: Option<Decimal>,
}

impl From<TokenTvlTotal> for TokenTvlPageTotalResponse {
    fn from(t: TokenTvlTotal) -> Self {
        Self {
            tvl_usd: t.tvl_usd,
            locked_usd: t.locked_usd,
        }
    }
}
