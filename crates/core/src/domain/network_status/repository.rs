//! Network status repository trait.
//!
//! The domain-level contract for persisting and reading the current
//! network status snapshot. The concrete Postgres implementation
//! lives in the `persistence` crate.
//!
//! The underlying table is a singleton (one row), so there is no
//! notion of querying "a" status — there is only ever *the* current
//! one.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::NetworkStatus};

/// Persistence contract for the network status snapshot.
#[async_trait]
pub trait NetworkStatusRepository: Send + Sync {
    /// Overwrite the current snapshot.
    ///
    /// Called by the indexer on every tick (~15s). Implementations
    /// upsert the singleton row — there is never more than one.
    async fn upsert(&self, status: &NetworkStatus) -> RepositoryResult<()>;

    /// Read the current snapshot.
    ///
    /// Called by the API for the dashboard's "Solana Live" panel.
    /// The singleton row is seeded by the migration, so a healthy
    /// system always has a row to return; `None` would indicate the
    /// seed row is missing (e.g. migration not applied).
    async fn get(&self) -> RepositoryResult<Option<NetworkStatus>>;
}
