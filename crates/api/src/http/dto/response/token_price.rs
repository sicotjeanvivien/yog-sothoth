use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

use super::token::PriceProviderResponse;
use crate::http::dto::schema::UnsignedDecimal;

/// The price block embedded in `TokenResponse`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenPriceResponse {
    /// USD price.
    #[schema(value_type = UnsignedDecimal)]
    pub(super) usd: Decimal,

    /// Origin tag: "jupiter" | "helius" | "fallback".
    pub(super) provider: PriceProviderResponse,

    /// When the price was observed.
    pub(super) fetched_at: DateTime<Utc>,
}
