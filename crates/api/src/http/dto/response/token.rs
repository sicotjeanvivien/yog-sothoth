//! Response DTO for `GET /api/tokens/{mint}`.
//!
//! Combines the token's identity (from `token_metadata`) with its
//! most recent price (from `token_prices`). The price block is
//! optional: a mint may have metadata but no price yet (very fresh,
//! flagged by Jupiter, or no Jupiter call has succeeded for it).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

use yog_core::domain::{TokenMetadata, TokenPrice};

/// The token detail payload.
///
/// Mirrors the convention used by `PoolResponse` and the other
/// `*Response` DTOs: a flat camelCase JSON shape, the domain types
/// never leak through.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenResponse {
    /// SPL mint address, base58.
    mint: String,

    /// Token symbol (e.g. "USDC"). `null` when DAS returned no
    /// Metaplex metadata.
    symbol: Option<String>,

    /// Token name (e.g. "USD Coin"). `null` for the same reason as
    /// `symbol`.
    name: Option<String>,

    /// Decimal precision of the mint.
    decimals: u8,

    /// Logo URI, as returned by DAS. May be an `ipfs://` URI.
    logo_uri: Option<String>,

    /// When the metadata was first fetched.
    fetched_at: DateTime<Utc>,

    /// When the metadata was last refreshed.
    last_refresh_at: DateTime<Utc>,

    /// Latest price observation, or `null` if the mint has never
    /// been priced.
    price: Option<TokenPriceResponse>,
}

/// The price block embedded in `TokenResponse`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenPriceResponse {
    /// USD price. Serialised as a JSON number — `rust_decimal`'s
    /// default Serialize uses an exact decimal representation.
    usd: Decimal,

    /// Origin tag: "jupiter" | "helius" | "fallback".
    source: String,

    /// When the price was observed.
    fetched_at: DateTime<Utc>,
}

impl TokenResponse {
    /// Assemble the response from its two sources.
    pub(crate) fn new(metadata: TokenMetadata, price: Option<TokenPrice>) -> Self {
        Self {
            mint: metadata.mint.to_string(),
            symbol: metadata.symbol,
            name: metadata.name,
            decimals: metadata.decimals,
            logo_uri: metadata.logo_uri,
            fetched_at: metadata.fetched_at,
            last_refresh_at: metadata.last_refresh_at,
            price: price.map(TokenPriceResponse::from),
        }
    }
}

impl From<TokenPrice> for TokenPriceResponse {
    fn from(price: TokenPrice) -> Self {
        Self {
            usd: price.price_usd,
            source: price.price_source.as_str().to_string(),
            fetched_at: price.fetched_at,
        }
    }
}
