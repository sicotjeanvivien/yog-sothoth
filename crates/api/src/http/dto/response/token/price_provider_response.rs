use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// Where a price came from.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PriceProviderResponse {
    Jupiter,
    Helius,
    Fallback,
}

impl From<domain::PriceProvider> for PriceProviderResponse {
    fn from(provider: domain::PriceProvider) -> Self {
        match provider {
            domain::PriceProvider::Jupiter => Self::Jupiter,
            domain::PriceProvider::Helius => Self::Helius,
            domain::PriceProvider::Fallback => Self::Fallback,
        }
    }
}
