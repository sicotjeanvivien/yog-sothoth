use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

/// The price block embedded in `TokenResponse`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenPriceResponse {
    /// USD price. Serialised as a JSON number — `rust_decimal`'s
    /// default Serialize uses an exact decimal representation.
    pub(super) usd: Decimal,

    /// Origin tag: "jupiter" | "helius" | "fallback".
    pub(super) provider: String,

    /// When the price was observed.
    pub(super) fetched_at: DateTime<Utc>,
}
