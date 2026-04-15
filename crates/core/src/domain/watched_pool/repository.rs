use crate::{domain::WatchedPool, CoreResult};
use async_trait::async_trait;

/// Interface for watched pool persistence.
#[async_trait]
pub trait WatchedPoolRepository: Send + Sync {
    /// Insert a new pool into the watchlist.
    async fn add(&self, pool: &WatchedPool) -> CoreResult<()>;

    /// Check if a pool is already being watched.
    async fn exists(&self, address: &str) -> CoreResult<bool>;

    /// Retrieve all watched pools.
    async fn find_all(&self) -> CoreResult<Vec<WatchedPool>>;

    /// Remove a pool from the watchlist.
    async fn remove(&self, pool: &str) -> CoreResult<()>;
}
