//! Application-level aggregate composing a pool with everything
//! needed to present it: both token sides (metadata + latest price)
//! and its derived analytics.
//!
//! This type is domain-facing — it carries `yog-core` types only and
//! is free of any HTTP/wire concern. The HTTP layer maps it to its
//! own `PoolResponse` DTO.

use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryResult,
    domain::{
        Pool, PoolAnalytics, PoolProperties, SignalRecord, TokenMetadata, TokenMetadataLookup,
        TokenPrice, TokenPriceLookup,
    },
};

/// One token side of a pool, enriched with whatever context is
/// available. Metadata and price are independently optional: a freshly
/// observed mint may have neither yet (yog-context hasn't caught up),
/// or metadata without a price (priced lazily).
#[derive(Debug, Clone)]
pub(crate) struct EnrichedToken {
    /// `None` until the pool's mints are resolved by yog-context.
    pub(crate) mint: Option<solana_pubkey::Pubkey>,
    pub(crate) metadata: Option<TokenMetadata>,
    pub(crate) price: Option<TokenPrice>,
}

impl EnrichedToken {
    /// The unresolved side: nothing to look up, nothing looked up.
    pub(crate) fn unresolved() -> Self {
        Self {
            mint: None,
            metadata: None,
            price: None,
        }
    }

    /// Resolve one token side from its mint: metadata and latest price,
    /// each independently absent when the enrichment pipeline hasn't
    /// caught up. A `None` mint short-circuits to [`unresolved`] — a
    /// pool discovered but not yet resolved by yog-context has nothing
    /// to look up. Shared by every service that embeds a token in
    /// context (pools, signals).
    ///
    /// [`unresolved`]: Self::unresolved
    pub(crate) async fn resolve(
        mint: Option<solana_pubkey::Pubkey>,
        metadata_repo: &dyn TokenMetadataLookup,
        price_repo: &dyn TokenPriceLookup,
    ) -> RepositoryResult<Self> {
        let Some(mint) = mint else {
            return Ok(Self::unresolved());
        };
        let metadata = metadata_repo.find_by_mint(&mint).await?;
        let price = price_repo.find_latest_by_mint(&mint).await?;
        Ok(Self {
            mint: Some(mint),
            metadata,
            price,
        })
    }
}

/// A pool composed with its two enriched token sides and its
/// pre-computed analytics. Produced by `PoolService`, consumed by the
/// HTTP layer for DTO mapping.
pub(crate) struct EnrichedPool {
    pub(crate) pool: Pool,
    pub(crate) token_a: EnrichedToken,
    pub(crate) token_b: EnrichedToken,
    pub(crate) analytics: PoolAnalytics,
    /// The pool's signals over the indicator window (newest first,
    /// per-pool capped) — powers the signal icon on the pools list.
    /// Empty when the pool emitted nothing in the window.
    pub(crate) recent_signals: Vec<SignalRecord>,
}

/// An [`EnrichedPool`] plus the properties specific to its protocol — the
/// aggregate behind the pool *detail* sheet.
///
/// Separate from [`EnrichedPool`] on purpose: the protocol-specific block costs
/// one extra query per pool, and the list endpoints neither need it nor display
/// it. Keeping it out of `EnrichedPool` means the paginated listing does not pay
/// for it.
pub(crate) struct EnrichedPoolDetail {
    pub(crate) pool: EnrichedPool,
    /// The pool's protocol-specific properties, still in domain form — the
    /// variant is the protocol's. `None` for a protocol that stores none, or for
    /// a pool with no satellite row yet (discovered, but neither enriched nor
    /// seen at genesis).
    ///
    /// Kept as the [`PoolProperties`] enum rather than one protocol's type so
    /// that nothing between the repository and the wire DTO has to know which
    /// protocol this is.
    pub(crate) properties: Option<PoolProperties>,

    /// When this read happened, used to place a fee scheduler on its decay
    /// curve.
    ///
    /// Carried rather than taken with `Utc::now()` at serialization time for one
    /// reason: a DTO that reads the clock cannot be asserted on. The service
    /// fills it once per request, and a test pins it.
    pub(crate) evaluated_at: DateTime<Utc>,
}
