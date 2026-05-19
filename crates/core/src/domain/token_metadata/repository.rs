//! Token metadata repository trait.
//!
//! Persistence contract for `token_metadata`. Placed in `domain`
//! alongside the other repository traits, per the crate convention.

use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::TokenMetadata};

/// Persistence contract for token metadata.
#[async_trait]
pub trait TokenMetadataRepository: Send + Sync {
    /// Insert or update the metadata for a mint.
    ///
    /// Called by the `yog-context` metadata worker after a DAS fetch.
    /// Implementations upsert on the `mint` primary key.
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()>;

    /// List the mints that already have a metadata row.
    ///
    /// Used by the price worker as the set of mints to price.
    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>>;

    /// List the mints seen in `pools` (token A or token B) that do
    /// NOT yet have a `token_metadata` row.
    ///
    /// This is the metadata worker's work queue: on each poll cycle
    /// it fetches DAS for these mints and upserts them. The query
    /// reads `pools`, but its purpose is metadata enrichment, so the
    /// method belongs to this repository rather than `PoolRepository`.
    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>>;
}
