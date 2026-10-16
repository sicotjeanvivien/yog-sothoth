//! Contracts of the `token_metadata` table.

use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::TokenMetadata};

/// Enrichment side of token metadata, yog-context's lens.
#[async_trait]
pub trait TokenMetadataRepository: Send + Sync {
    /// Insert or update the metadata of a mint.
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()>;

    /// The mints that have a metadata row: what the price worker prices.
    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>>;

    /// The mints of `pools` without a metadata row: the metadata worker's queue.
    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>>;
}

/// Metadata reads by mint, the api's lens.
#[async_trait]
pub trait TokenMetadataLookup: Send + Sync {
    /// The metadata of a mint, `None` before its first fetch.
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>>;
}
