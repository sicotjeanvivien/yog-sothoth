//! Response DTO for `GET /api/tokens/{mint}`.
//!
//! Combines the token's identity (from `token_metadata`) with its
//! most recent price (from `token_prices`). The price block is
//! optional: a mint may have metadata but no price yet (very fresh,
//! flagged by Jupiter, or no Jupiter call has succeeded for it).

mod embedded_token_response;
mod price_provider_response;
mod price_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{TokenMetadata, TokenPrice};

pub(super) use embedded_token_response::EmbeddedTokenResponse;
use price_response::TokenPriceResponse;

/// The token detail payload.
#[derive(Debug, Serialize, ToSchema)]
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

    /// Logo URL, as returned by DAS. `null` unless it is `http` or `https`.
    #[schema(format = "uri")]
    logo_uri: Option<String>,

    /// When the metadata was first fetched.
    fetched_at: DateTime<Utc>,

    /// When the metadata was last refreshed.
    last_refresh_at: DateTime<Utc>,

    /// Latest price observation, or `null` if the mint has never
    /// been priced.
    price: Option<TokenPriceResponse>,
}

impl TokenResponse {
    /// Assemble the response from its two sources.
    pub(crate) fn new(metadata: TokenMetadata, price: Option<TokenPrice>) -> Self {
        Self {
            mint: metadata.mint.to_string(),
            symbol: metadata.symbol,
            name: metadata.name,
            decimals: metadata.decimals,
            logo_uri: logo_uri(metadata.logo_uri),
            fetched_at: metadata.fetched_at,
            last_refresh_at: metadata.last_refresh_at,
            price: price.map(TokenPriceResponse::from),
        }
    }
}

/// Only an `http`/`https` URL is kept: under another scheme `url` accepts what
/// the web's `z.url()` refuses, and a browser shows neither `ipfs:` nor `blob:`.
fn logo_uri(raw: Option<String>) -> Option<String> {
    // ⚠️ No `trim()`: Rust's strips U+0085, JS's keeps it, so a trimmed
    // value would pass here and fail on the web.
    raw.filter(|s| url::Url::parse(s).is_ok_and(|u| matches!(u.scheme(), "http" | "https")))
}

#[cfg(test)]
#[path = "tests/token_tests.rs"]
mod tests;
