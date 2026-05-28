//! Application-level aggregate composing a pool with everything
//! needed to present it: both token sides (metadata + latest price)
//! and its derived analytics.
//!
//! This type is domain-facing — it carries `yog-core` types only and
//! is free of any HTTP/wire concern. The HTTP layer maps it to its
//! own `PoolResponse` DTO.

use yog_core::domain::{Pool, PoolAnalytics, TokenMetadata, TokenPrice};

/// One token side of a pool, enriched with whatever context is
/// available. Metadata and price are independently optional: a freshly
/// observed mint may have neither yet (yog-context hasn't caught up),
/// or metadata without a price (priced lazily).
pub(crate) struct EnrichedToken {
    pub(crate) mint: solana_pubkey::Pubkey,
    pub(crate) metadata: Option<TokenMetadata>,
    pub(crate) price: Option<TokenPrice>,
}

/// A pool composed with its two enriched token sides and its
/// pre-computed analytics. Produced by `PoolService`, consumed by the
/// HTTP layer for DTO mapping.
pub(crate) struct EnrichedPool {
    pub(crate) pool: Pool,
    pub(crate) token_a: EnrichedToken,
    pub(crate) token_b: EnrichedToken,
    pub(crate) analytics: PoolAnalytics,
}
