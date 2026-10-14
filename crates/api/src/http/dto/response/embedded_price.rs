use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

use crate::http::dto::schema::UnsignedDecimal;

/// The price block inside an embedded token. Same shape and intent
/// as the one in `TokenResponse`, kept here to keep the embedded
/// view self-contained.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmbeddedPriceResponse {
    /// USD price.
    #[schema(value_type = UnsignedDecimal)]
    pub(super) usd: Decimal,
    /// Origin tag: "jupiter" | "helius" | "fallback".
    pub(super) provider: PriceProvider,
    /// When the price was observed.
    pub(super) fetched_at: DateTime<Utc>,
}

/// Where a price came from.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PriceProvider {
    Jupiter,
    Helius,
    Fallback,
}

impl From<domain::PriceProvider> for PriceProvider {
    fn from(provider: domain::PriceProvider) -> Self {
        match provider {
            domain::PriceProvider::Jupiter => Self::Jupiter,
            domain::PriceProvider::Helius => Self::Helius,
            domain::PriceProvider::Fallback => Self::Fallback,
        }
    }
}
