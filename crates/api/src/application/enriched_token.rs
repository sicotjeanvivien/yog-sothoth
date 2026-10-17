//! One token side of a pool, in `yog-core` types; the HTTP layer maps it to
//! `EmbeddedTokenResponse`.

use yog_core::{
    RepositoryResult,
    domain::{TokenMetadata, TokenMetadataLookup},
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
