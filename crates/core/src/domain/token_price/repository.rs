//! Token price repository trait.
//!
//! Persistence contract for the `token_prices` hypertable. Placed in
//! `domain` alongside the other repository traits.

use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::TokenPrice};

/// Persistence contract for token prices — the write side, owned by
/// yog-context. The read side lives in [`TokenPriceLookup`].
#[async_trait]
pub trait TokenPriceRepository: Send + Sync {
    /// Insert a batch of price observations.
    ///
    /// Called by the `yog-context` price worker on each interval tick
    /// after a Jupiter fetch. A batch insert keeps the per-tick write
    /// to a single round-trip. `token_prices` is append-only — each
    /// observation is a new row keyed by `(mint, fetched_at)`.
    async fn insert_batch(&self, prices: &[TokenPrice]) -> RepositoryResult<()>;
}

/// Latest-price consultation — the api's lens.
///
/// Kept separate from [`TokenPriceRepository`] (write side, context) so
/// each binary depends on exactly the methods it uses.
#[async_trait]
pub trait TokenPriceLookup: Send + Sync {
    /// Fetch the most recent price observation for a mint, or `None`
    /// if the mint has never been priced. Used by the
    /// `GET /api/tokens/{mint}` handler.
    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>>;
}
