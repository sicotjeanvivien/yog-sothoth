//! Token price repository trait.
//!
//! Persistence contract for the `token_prices` hypertable. Placed in
//! `domain` alongside the other repository traits.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::TokenPrice};

/// Persistence contract for token prices.
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
