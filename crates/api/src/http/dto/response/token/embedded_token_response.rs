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
//!     in `PoolResponse`. It carries the token's identity only: symbol,
//!     name, logo, decimals — never its price.
//!
//! Re-use this DTO wherever a token needs to appear inside another
//! resource (e.g. later, in `SwapEventResponse` to show
//! "1.2 SOL → 240 USDC").
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::TokenMetadata;

use super::logo_uri;
use crate::application::EnrichedToken;

/// A token, embedded in another resource's response.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(in crate::http::dto::response) struct EmbeddedTokenResponse {
    /// SPL mint address, base58. `null` until the pool's mints are
    /// resolved by yog-context.
    mint: Option<String>,

    /// Symbol (e.g. "USDC"). `null` when DAS returned no metadata.
    symbol: Option<String>,

    /// Display name (e.g. "USD Coin"). `null` for the same reason
    /// as `symbol`.
    name: Option<String>,

    /// Decimal precision.
    decimals: u8,

    /// Logo URL as returned by DAS. `null` unless it is `http` or `https`.
    #[schema(format = "uri")]
    logo_uri: Option<String>,
}

impl EmbeddedTokenResponse {
    /// Without metadata the response falls back to a minimal view keyed
    /// only by `mint`.
    pub(in crate::http::dto::response) fn from_sources(
        mint: Option<solana_pubkey::Pubkey>,
        metadata: Option<TokenMetadata>,
    ) -> Self {
        // If metadata is present, use it; otherwise fall back to
        // showing just the mint with nulls — the dashboard can
        // render a placeholder rather than hiding the pool.
        let (symbol, name, decimals, logo_uri) = match metadata {
            Some(m) => (m.symbol, m.name, m.decimals, logo_uri(m.logo_uri)),
            None => (None, None, 0, None),
        };

        Self {
            mint: mint.map(|m| m.to_string()),
            symbol,
            name,
            decimals,
            logo_uri,
        }
    }
}

impl From<EnrichedToken> for EmbeddedTokenResponse {
    fn from(t: EnrichedToken) -> Self {
        EmbeddedTokenResponse::from_sources(t.mint, t.metadata)
    }
}
