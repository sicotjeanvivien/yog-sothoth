//! A pool composed with its two token sides (identity only) and its derived
//! analytics, in `yog-core` types; the HTTP layer maps it to `PoolResponse`.

use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryResult,
    domain::{
        Pool, PoolAnalytics, PoolProperties, SignalRecord, TokenMetadata, TokenMetadataLookup,
    },
};

/// One token side of a pool: its identity, without its price. Metadata is
/// `None` until yog-context resolves the mint.
#[derive(Debug, Clone)]
pub(crate) struct EnrichedToken {
    /// `None` until the pool's mints are resolved by yog-context.
    pub(crate) mint: Option<solana_pubkey::Pubkey>,
    pub(crate) metadata: Option<TokenMetadata>,
}

impl EnrichedToken {
    /// The unresolved side: nothing to look up, nothing looked up.
    pub(crate) fn unresolved() -> Self {
        Self {
            mint: None,
            metadata: None,
        }
    }

    /// Resolve one token side from its mint. A `None` mint short-circuits
    /// to [`unresolved`].
    ///
    /// [`unresolved`]: Self::unresolved
    pub(crate) async fn resolve(
        mint: Option<solana_pubkey::Pubkey>,
        metadata_repo: &dyn TokenMetadataLookup,
    ) -> RepositoryResult<Self> {
        let Some(mint) = mint else {
            return Ok(Self::unresolved());
        };
        let metadata = metadata_repo.find_by_mint(&mint).await?;
        Ok(Self {
            mint: Some(mint),
            metadata,
        })
    }
}

/// A pool with its two enriched token sides and its analytics. `Clone`: the ranked list is
/// served to every caller of the top-pools cache.
#[derive(Clone)]
pub(crate) struct EnrichedPool {
    pub(crate) pool: Pool,
    pub(crate) token_a: EnrichedToken,
    pub(crate) token_b: EnrichedToken,
    pub(crate) analytics: PoolAnalytics,
    /// The pool's signals over the indicator window, newest first, capped per pool.
    pub(crate) recent_signals: Vec<SignalRecord>,
}

/// An [`EnrichedPool`] plus its protocol-specific properties — the detail sheet. Kept apart:
/// the block costs a query per pool that the list endpoints do not pay.
pub(crate) struct EnrichedPoolDetail {
    pub(crate) pool: EnrichedPool,
    /// The protocol-specific properties, as the [`PoolProperties`] enum so nothing between the
    /// repository and the DTO names the protocol. `None` without a satellite row.
    pub(crate) properties: Option<PoolProperties>,

    /// When this read happened, to place a fee scheduler on its curve. Carried, not read
    /// at serialization: a DTO that reads the clock cannot be asserted on.
    pub(crate) evaluated_at: DateTime<Utc>,
}
