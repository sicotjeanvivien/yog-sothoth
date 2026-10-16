use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::TokenPrice;

use super::price_provider_response::PriceProviderResponse;
use crate::http::dto::schema::UnsignedDecimal;

/// A token's latest price, in `TokenResponse`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct TokenPriceResponse {
    /// USD price.
    #[schema(value_type = UnsignedDecimal)]
    usd: Decimal,

    /// Origin tag: "jupiter" | "helius" | "fallback".
    provider: PriceProviderResponse,

    /// When the price was observed.
    fetched_at: DateTime<Utc>,
}

impl From<TokenPrice> for TokenPriceResponse {
    fn from(price: TokenPrice) -> Self {
        Self {
            usd: price.price_usd,
            provider: price.price_provider.into(),
            fetched_at: price.fetched_at,
        }
    }
}
