//! Contracts of the singleton `network_status` row.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::NetworkStatus};

/// Write side of the snapshot, the indexer reporter's lens.
#[async_trait]
pub trait NetworkStatusRepository: Send + Sync {
    /// Overwrite the singleton row.
    async fn upsert(&self, status: &NetworkStatus) -> RepositoryResult<()>;
}

/// Reading the snapshot, the api's lens.
#[async_trait]
pub trait NetworkStatusLookup: Send + Sync {
    /// The current snapshot; `None` means the migration's seed row is missing.
    async fn get(&self) -> RepositoryResult<Option<NetworkStatus>>;
}
