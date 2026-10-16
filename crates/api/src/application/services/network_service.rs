//! Application service for the chain's position as the probe last saw it.
//!
//! Thin by design: the snapshot is a singleton row the indexer's reporter
//! overwrites; the service only reads it.

use std::sync::Arc;

use yog_core::{
    RepositoryError,
    domain::{NetworkStatus, NetworkStatusLookup},
};

pub(crate) struct NetworkService {
    network_status_repo: Arc<dyn NetworkStatusLookup>,
}

impl NetworkService {
    pub(crate) fn new(network_status_repo: Arc<dyn NetworkStatusLookup>) -> Self {
        Self {
            network_status_repo,
        }
    }

    /// The latest snapshot, `None` when the seeded singleton row is missing.
    pub(crate) async fn get(&self) -> Result<Option<NetworkStatus>, RepositoryError> {
        self.network_status_repo.get().await
    }
}

#[cfg(test)]
#[path = "tests/network_service_tests.rs"]
mod tests;
