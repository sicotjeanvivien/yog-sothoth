//! Repository contract for the [`PoolCurrentState`] projection.
//!
//! The implementation lives in `crates/indexer/src/repositories/`. Keeping the
//! trait in `core` lets `api` consume the projection without depending on
//! sqlx/Postgres.

use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{PoolCurrentState, PoolCurrentStateUpsert},
};

/// Read/write access to the pool-current-state projection.
///
/// # Contract
///
/// * [`upsert`](Self::upsert) is **stale-write safe**: the implementation MUST
///   ignore an upsert whose `event_at` is strictly older than the value
///   already stored. This makes replay and out-of-order processing safe
///   without requiring the caller to coordinate ordering.
///
/// * [`upsert`](Self::upsert) MUST preserve `last_sqrt_price` / `last_swap_at`
///   when the incoming payload is a liquidity event (i.e. `sqrt_price`
///   is `None`), and conversely preserve `liquidity` / `last_liquidity_at`
///   when the payload is a swap event.
///
/// * [`upsert`](Self::upsert) MUST refresh `updated_at` to `NOW()` on every
///   successful write (whether or not the stale-write guard applied).
#[async_trait]
pub trait PoolCurrentStateRepository: Send + Sync {
    /// Apply an event-derived state update to the projection.
    ///
    /// Returns `Ok(true)` when the row was updated (or inserted),
    /// `Ok(false)` when the stale-write guard suppressed the update.
    async fn upsert(&self, upsert: &PoolCurrentStateUpsert) -> RepositoryResult<bool>;

    /// Fetch the current state of a single pool, or `Ok(None)` if no event
    /// has been observed for it yet.
    async fn get_by_address(
        &self,
        pool_address: &str,
    ) -> RepositoryResult<Option<PoolCurrentState>>;

    /// List pools sorted by most-recent activity first.
    ///
    /// `limit` is the page size and MUST be > 0. `before_last_event_at`, when
    /// set, restricts to rows strictly older than the given instant — used as
    /// the cursor in keyset pagination.
    async fn list_most_recent(
        &self,
        limit: u32,
        before_last_event_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> RepositoryResult<Vec<PoolCurrentState>>;
}
