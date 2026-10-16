//! A token embedded in another resource's response, such as a pool: its identity only —
//! symbol, name, logo, decimals, never its price. `TokenResponse` is the primary resource.
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
