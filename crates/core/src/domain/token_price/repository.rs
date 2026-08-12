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
    ///
    /// # Contract: every price must satisfy [`TokenPrice::is_storable`]
    ///
    /// **The caller filters; this method does not.** The batch goes to the
    /// database as ONE statement, so a single row the `price_usd` column cannot
    /// hold takes the whole batch with it — `23514` from the `CHECK` of
    /// migration 009 below `5e-19`, `22003` from the column type itself at or
    /// above `1e20`, and `ON CONFLICT DO NOTHING` covers neither. Every other
    /// mint in that tick is lost, and migration 005 established that the
    /// resulting as-of price gap never heals: nothing backfills history.
    ///
    /// So this is not a preference about where validation lives. Passing an
    /// unstorable price does not degrade one row — it silently drops the tick,
    /// permanently, and repeats for as long as that mint is priced. A new writer
    /// (a backfill, a second price source, a repair task) must apply the same
    /// filter; the debug assertion in the Postgres implementation is there to
    /// make forgetting it fail loudly in dev rather than quietly in production.
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
