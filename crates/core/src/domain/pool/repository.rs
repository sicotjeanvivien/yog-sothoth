use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::Pool};

/// Persistence contract for Pool.
///
/// Implemented by the infrastructure layer (`infra::db`).
/// `core` defines the interface, the indexer wires the concrete implementation.   
#[async_trait]
pub trait PoolRepository: Send + Sync {
    /// Insert a new pool, or refresh an existing one's `last_seen_at`.
    /// Used when an event arrives that fully describes the pool
    /// (Swap, Liquidity).
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()>;

    /// Refresh `last_seen_at` for an existing pool — but do NOT insert
    /// the row if the pool is unknown.
    ///
    /// Used by events that touch a pool without carrying enough info
    /// to populate it (ClaimPositionFee, ClaimReward — these don't
    /// expose the pool's mint addresses). If the pool isn't yet known,
    /// the call is a no-op; the next Swap or Liquidity event will
    /// create the row properly.
    async fn touch_last_seen(&self, pool_address: &Pubkey) -> RepositoryResult<()>;
}
