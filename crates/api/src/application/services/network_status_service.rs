//! Application service for network status.
//!
//! Assembles two reads — the persisted slot/latency snapshot and the
//! ingestion freshness signal — into a single domain aggregate.
//! The freshness rule (thresholds) lives in `core`; this service
//! only orchestrates the reads.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{EventFreshnessRepository, FreshnessStatus, NetworkStatus, NetworkStatusRepository},
};

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

/// The assembled network status: both sources combined.
#[derive(Debug)]
pub(crate) struct NetworkStatusAggregate {
    pub status: NetworkStatus,
    pub freshness: FreshnessStatus,
    pub last_event_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for network status queries.
pub(crate) struct NetworkStatusService {
    network_status_repo: Arc<dyn NetworkStatusRepository>,
    event_freshness_repo: Arc<dyn EventFreshnessRepository>,
}

impl NetworkStatusService {
    pub(crate) fn new(
        network_status_repo: Arc<dyn NetworkStatusRepository>,
        event_freshness_repo: Arc<dyn EventFreshnessRepository>,
    ) -> Self {
        Self {
            network_status_repo,
            event_freshness_repo,
        }
    }

    /// Assemble the current network status.
    ///
    /// Returns `Ok(None)` when the `network_status` singleton row is
    /// missing (migration not applied or seed row deleted). The
    /// handler maps this to an internal error.
    pub(crate) async fn get_status(
        &self,
    ) -> Result<Option<NetworkStatusAggregate>, RepositoryError> {
        let Some(status) = self.network_status_repo.get().await? else {
            return Ok(None);
        };

        let last_event_at = self.event_freshness_repo.last_event_at().await?;
        let freshness = FreshnessStatus::from_last_event(last_event_at, Utc::now());

        Ok(Some(NetworkStatusAggregate {
            status,
            freshness,
            last_event_at,
        }))
    }
}

#[cfg(test)]
#[path = "tests/network_status_service_tests.rs"]
mod tests;
