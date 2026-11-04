//! One token side of a pool, in `yog-core` types; the HTTP layer maps it to
//! `EmbeddedTokenResponse`.

use yog_core::domain::TokenMetadata;

/// One token side of a pool: its identity, without its price. Metadata is
/// `None` until yog-context resolves the mint.
#[derive(Debug, Clone)]
pub(crate) struct EnrichedToken {
    /// `None` until the pool's mints are resolved by yog-context.
    pub(crate) mint: Option<solana_pubkey::Pubkey>,
    pub(crate) metadata: Option<TokenMetadata>,
}

impl EnrichedToken {
    pub(crate) fn unresolved() -> Self {
        Self {
            mint: None,
            metadata: None,
        }
    }
}
