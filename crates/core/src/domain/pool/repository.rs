use async_trait::async_trait;

use crate::{domain::Pool, CoreResult};

/// Persistence contract for Pool.
///
/// Implemented by the infrastructure layer (`infra::db`).
/// `core` defines the interface, the indexer wires the concrete implementation.
#[async_trait]
pub trait PoolRepository: Send + Sync {
    /// Insert or refresh a pool's last_seen_at timestamp.
    ///
    /// On first observation, inserts the full row.
    /// On subsequent observations, updates `last_seen_at` only.
    async fn upsert(&self, pool: &Pool) -> CoreResult<()>;
}
