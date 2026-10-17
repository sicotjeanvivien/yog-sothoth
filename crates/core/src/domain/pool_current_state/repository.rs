//! Repository contracts of the `pool_current_state` projection.

use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{PoolCurrentStateUpsert, PoolCurrentStateUpsertOutcome},
};

/// Write access to the projection, the indexer's lens.
/// ⚠️ `upsert` ignores a position not strictly after the stored one, by
/// `(slot, transaction_index, event_index)`: never by `event_at`, a shared second.
#[async_trait]
pub trait PoolCurrentStateRepository: Send + Sync {
    /// Apply an event-derived update, refreshing `updated_at`; a liquidity payload
    /// keeps `last_sqrt_price` and `last_swap_at`; a same-slot ambiguity is reported.
    async fn upsert(
        &self,
        upsert: &PoolCurrentStateUpsert,
    ) -> RepositoryResult<PoolCurrentStateUpsertOutcome>;
}
