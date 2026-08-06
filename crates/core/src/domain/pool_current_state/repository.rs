//! Repository contract for the [`PoolCurrentState`] projection.
//!
//! The implementation lives in `crates/indexer/src/repositories/`. Keeping the
//! trait in `core` lets `api` consume the projection without depending on
//! sqlx/Postgres.

use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{PoolCurrentState, PoolCurrentStateUpsert, PoolCurrentStateUpsertOutcome},
};

/// Write access to the pool-current-state projection — the indexer's lens.
///
/// The read side lives in [`PoolCurrentStateLookup`].
///
/// # Contract
///
/// * [`upsert`](Self::upsert) is **out-of-order safe**: the implementation
///   MUST ignore an upsert whose position is not strictly after the one
///   already stored, comparing `(slot, transaction_index, event_index)` as a
///   tuple. This makes replay and out-of-order processing safe without
///   requiring the caller to coordinate ordering.
///
///   It MUST NOT order on `event_at`: that timestamp comes from `blockTime`
///   and has second granularity, which 56 % of swaps share with another swap
///   of the same pool. Ordering on it rejected a third of all updates.
///
/// * [`upsert`](Self::upsert) MUST report a `same_slot_ambiguity` when the
///   state it met came from the same slot under a different signature — the
///   residual case the reachable key cannot rank (see
///   [`PoolCurrentStateUpsertOutcome`]).
///
/// * [`upsert`](Self::upsert) MUST preserve `last_sqrt_price` / `last_swap_at`
///   when the incoming payload is a liquidity event (i.e. `sqrt_price`
///   is `None`). That pair is the only kind-specific state the projection
///   still carries: the liquidity side used to hold a `liquidity` /
///   `last_liquidity_at` pair, dropped in migration 003 because the value was
///   a position's delta, not the pool's L.
///
/// * [`upsert`](Self::upsert) MUST refresh `updated_at` to `NOW()` on every
///   successful write (whether or not the stale-write guard applied).
#[async_trait]
pub trait PoolCurrentStateRepository: Send + Sync {
    /// Apply an event-derived state update to the projection.
    async fn upsert(
        &self,
        upsert: &PoolCurrentStateUpsert,
    ) -> RepositoryResult<PoolCurrentStateUpsertOutcome>;
}

/// Consultation of the pool-current-state projection — the api's lens.
///
/// Kept separate from [`PoolCurrentStateRepository`] (write side, indexer)
/// so each binary depends on exactly the methods it uses.
#[async_trait]
pub trait PoolCurrentStateLookup: Send + Sync {
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
