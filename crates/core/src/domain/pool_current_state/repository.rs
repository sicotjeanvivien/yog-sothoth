//! Repository contracts of the [`PoolCurrentState`] projection.

use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{
    RepositoryResult,
    domain::{
        PoolCurrentState, PoolCurrentStateUpsert, PoolCurrentStateUpsertOutcome, PoolSideValues,
    },
};

/// Write access to the projection, the indexer's lens.
/// ⚠️ `upsert` ignores a position not strictly after the stored one, by
/// `(slot, transaction_index, event_index)`: never by `event_at`, a shared second.
#[async_trait]
pub trait PoolCurrentStateRepository: Send + Sync {
    /// Apply an event-derived update; a liquidity payload keeps `last_sqrt_price`
    /// and `last_swap_at`, and a same-slot ambiguity is reported.
    async fn upsert(
        &self,
        upsert: &PoolCurrentStateUpsert,
    ) -> RepositoryResult<PoolCurrentStateUpsertOutcome>;
}

/// Reading the projection, the api's lens.
#[async_trait]
pub trait PoolCurrentStateLookup: Send + Sync {
    /// The current state of a pool, `None` before its first event.
    async fn get_by_address(
        &self,
        pool_address: &str,
    ) -> RepositoryResult<Option<PoolCurrentState>>;

    /// The USD value of each side of a pool's reserves. Both `None` when the pool
    /// has no state yet or a token's metadata is unresolved.
    async fn side_values_usd(&self, pool_address: &Pubkey) -> RepositoryResult<PoolSideValues>;

    /// Pools by most recent activity; `limit` must be > 0, `before_last_event_at`
    /// is the keyset cursor.
    async fn list_most_recent(
        &self,
        limit: u32,
        before_last_event_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> RepositoryResult<Vec<PoolCurrentState>>;
}
