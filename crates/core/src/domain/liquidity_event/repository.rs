use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;

use crate::{domain::LiquidityEvent, CoreResult};

/// Persistence contract for liquidity events.
///
/// Implemented by the infrastructure layer (`infra::db`).
/// `core` defines the interface, the indexer wires the concrete implementation.
#[async_trait]
pub trait LiquidityEventRepository {
    /// Persist a liquidity event.
    async fn insert(&self, event: &LiquidityEvent) -> CoreResult<()>;

    /// Retrieve the most recent liquidity events for a pool, ordered by timestamp descending.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> CoreResult<Vec<LiquidityEvent>>;
}
