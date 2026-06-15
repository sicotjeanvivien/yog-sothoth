//! Embedded view of a token, used when a token appears in context
//! inside another resource's response (typically a pool).
//!
//! This is intentionally NOT the same shape as `TokenResponse`. The
//! two have different roles:
//!
//!   - `TokenResponse` is the "primary resource" view, returned by
//!     `GET /api/tokens/{mint}`. It carries everything a client
//!     might want to know about a token, including enrichment
//!     metadata (when the row was first fetched, when it was last
//!     refreshed).
//!   - `EmbeddedTokenResponse` is the "in-context" view, embedded
//!     in `PoolResponse`. It carries only what is needed to display
//!     the token in the parent resource's UI: symbol, name, logo,
//!     decimals, current price.
//!
//! Re-use this DTO wherever a token needs to appear inside another
//! resource (e.g. later, in `SwapEventResponse` to show
//! "1.2 SOL → 240 USDC").
use serde::Serialize;

use yog_core::domain::{TokenMetadata, TokenPrice};

use crate::http::dto::response::EmbeddedPriceResponse;

/// A token, embedded in another resource's response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmbeddedTokenResponse {
    /// SPL mint address, base58.
    mint: String,

    /// Symbol (e.g. "USDC"). `null` when DAS returned no metadata.
    symbol: Option<String>,

    /// Display name (e.g. "USD Coin"). `null` for the same reason
    /// as `symbol`.
    name: Option<String>,

    /// Decimal precision.
    decimals: u8,

    /// Logo URI as returned by DAS. May be an `ipfs://` URI.
    logo_uri: Option<String>,

    /// Latest known price observation, or `null` when the token has
    /// never been priced.
    price: Option<EmbeddedPriceResponse>,
}

impl EmbeddedTokenResponse {
    /// Build the embedded view from the two sources. Both arguments
    /// are `Option` because, when this is called during a pool
    /// enrichment, neither metadata nor price is guaranteed to
    /// exist yet — the response then falls back to a minimal view
    /// keyed only by the mint pubkey (passed in via the metadata's
    /// `mint` field when available, otherwise via the second arg).
    ///
    /// The caller (the pool handler) always knows the mint pubkey
    /// from the pool itself, so it can construct a meaningful
    /// response even when no metadata row exists yet.
    pub(crate) fn from_sources(
        mint: solana_pubkey::Pubkey,
        metadata: Option<TokenMetadata>,
        price: Option<TokenPrice>,
    ) -> Self {
        // If metadata is present, use it; otherwise fall back to
        // showing just the mint with nulls — the dashboard can
        // render a placeholder rather than hiding the pool.
        let (symbol, name, decimals, logo_uri) = match metadata {
            // Treat an empty logo as absent: the API contract is "URL or null".
            Some(m) => (
                m.symbol,
                m.name,
                m.decimals,
                m.logo_uri.filter(|s| !s.is_empty()),
            ),
            None => (None, None, 0, None),
        };

        Self {
            mint: mint.to_string(),
            symbol,
            name,
            decimals,
            logo_uri,
            price: price.map(EmbeddedPriceResponse::from),
        }
    }
}

impl From<TokenPrice> for EmbeddedPriceResponse {
    fn from(price: TokenPrice) -> Self {
        Self {
            usd: price.price_usd,
            provider: price.price_provider.as_str().to_string(),
            fetched_at: price.fetched_at,
        }
    }
}

#[cfg(test)]
#[path = "tests/embedded_token_tests.rs"]
mod tests;
