use crate::{RepositoryResult, domain::WatchedPool};
use async_trait::async_trait;

/// Interface for watched pool persistence.
#[async_trait]
pub trait WatchedPoolRepository: Send + Sync {
    /// Insert a new pool into the watchlist.
    async fn add(&self, pool: &WatchedPool) -> RepositoryResult<()>;

    /// Check if a pool is already being watched.
    async fn exists(&self, address: &str) -> RepositoryResult<bool>;

    /// Retrieve all watched pools.
    async fn find_all(&self) -> RepositoryResult<Vec<WatchedPool>>;

    /// Remove a pool from the watchlist.
    async fn remove(&self, pool: &str) -> RepositoryResult<()>;
}
