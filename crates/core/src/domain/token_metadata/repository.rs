//! Contracts of the `token_metadata` table.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::TokenMetadata};

/// How recent an event on one of its pools, or a price from the source, keeps a
/// mint worth asking: one quiet on both counts waits for its pools to move.
pub const PRICE_CANDIDATE_WINDOW: Duration = Duration::hours(24);

/// Enrichment side of token metadata, yog-context's lens.
#[async_trait]
pub trait TokenMetadataRepository: Send + Sync {
    /// Insert or update the metadata of a mint.
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()>;

    /// The mints with a metadata row that one of their pools moved, or that the
    /// source priced, since `since`: what the price worker asks.
    async fn list_price_candidates(&self, since: DateTime<Utc>) -> RepositoryResult<Vec<Pubkey>>;

    /// The mints of `pools` without a metadata row: the metadata worker's queue.
    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>>;
}

/// Metadata reads by mint, the api's lens.
#[async_trait]
pub trait TokenMetadataLookup: Send + Sync {
    /// The metadata of a mint, `None` before its first fetch.
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>>;
}
