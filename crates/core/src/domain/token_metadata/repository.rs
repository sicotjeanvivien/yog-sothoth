//! Contracts of the `token_metadata` table.

use async_trait::async_trait;
use chrono::Duration;
use solana_pubkey::Pubkey;

use crate::{
    Page, RepositoryResult,
    domain::{PriceCandidate, PriceCandidateParams, TokenCursor, TokenEntry, TokenMetadata},
};

/// How recent an event on one of its pools, or a price from the source, keeps a
/// mint worth asking: one quiet on both counts waits for its pools to move.
pub const PRICE_CANDIDATE_WINDOW: Duration = Duration::hours(24);

/// How recently a pool was discovered for its mints to be looked up at every metadata tick;
/// older pools wait for the sweep over all of them.
pub const MISSING_MINT_DISCOVERY_WINDOW: Duration = Duration::hours(1);

/// Enrichment side of token metadata, yog-context's lens.
#[async_trait]
pub trait TokenMetadataRepository: Send + Sync {
    /// Insert or update the metadata of a mint.
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()>;

    /// The mints with a metadata row that one of their pools moved within `params.window` of now,
    /// or that the source priced within it of the last price written, each with its class.
    async fn list_price_candidates(
        &self,
        params: &PriceCandidateParams,
    ) -> RepositoryResult<Vec<PriceCandidate>>;

    /// The mints without a metadata row of the pools discovered within `window` of now.
    async fn list_missing_mints_of_recent_pools(
        &self,
        window: Duration,
    ) -> RepositoryResult<Vec<Pubkey>>;

    /// The mints without a metadata row of every pool. ⚠️ Reads the whole of `pools`.
    async fn list_missing_mints_of_all_pools(&self) -> RepositoryResult<Vec<Pubkey>>;
}

/// Metadata reads by mint, the api's lens.
#[async_trait]
pub trait TokenMetadataLookup: Send + Sync {
    /// The metadata of a mint, `None` before its first fetch.
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>>;

    /// A page of tokens, the most recently first fetched first, ties by mint descending,
    /// after `cursor`'s key; forward only.
    async fn list_entries(
        &self,
        cursor: Option<TokenCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<TokenEntry>>;
}
