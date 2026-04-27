use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{domain::SwapEvent, RepositoryResult};

/// Persistence contract for swap events.
///
/// Implemented by the infrastructure layer (`infra::db`).
/// `core` defines the interface, the indexer wires the concrete implementation.
#[async_trait]
pub trait SwapEventRepository: Send + Sync {
    /// Persist a swap event.
    async fn insert(&self, event: &SwapEvent) -> RepositoryResult<()>;

    /// Retrieve the most recent swap events for a pool, ordered by timestamp descending.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<SwapEvent>>;
}
