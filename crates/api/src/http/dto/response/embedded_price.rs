use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

/// The price block inside an embedded token. Same shape and intent
/// as the one in `TokenResponse`, kept here to keep the embedded
/// view self-contained.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmbeddedPriceResponse {
    /// USD price.
    pub(super) usd: Decimal,
    /// Origin tag: "jupiter" | "helius" | "fallback".
    pub(super) source: String,
    /// When the price was observed.
    pub(super) fetched_at: DateTime<Utc>,
}
