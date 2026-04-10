use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{CoreResult, domain::SwapEvent};

/// Persistence contract for swap events.
///
/// Implemented by the infrastructure layer (`infra::db`).
/// `core` defines the interface, the indexer wires the concrete implementation.
#[async_trait]
pub trait SwapEventRepository {
    /// Persist a swap event.
    async fn insert(&self, event: &SwapEvent) -> CoreResult<()>;

    /// Retrieve the most recent swap events for a pool, ordered by timestamp descending.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> CoreResult<Vec<SwapEvent>>;
}