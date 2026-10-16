//! Application service for how fresh our ingestion is.
//!
//! The freshness rule (thresholds) lives in `core`; the service reads the last
//! indexed event and anchors the verdict to the request clock.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{EventFreshnessRepository, FreshnessStatus},
};

/// The freshness verdict and the event it was judged on.
#[derive(Debug)]
pub(crate) struct IngestionAggregate {
    pub freshness: FreshnessStatus,
    pub last_event_at: Option<DateTime<Utc>>,
}

pub(crate) struct IngestionService {
    event_freshness_repo: Arc<dyn EventFreshnessRepository>,
}

impl IngestionService {
    pub(crate) fn new(event_freshness_repo: Arc<dyn EventFreshnessRepository>) -> Self {
        Self {
            event_freshness_repo,
        }
    }

    /// Judge the last indexed event against now. No event at all is `Stale`.
    pub(crate) async fn freshness(&self) -> Result<IngestionAggregate, RepositoryError> {
        let last_event_at = self.event_freshness_repo.last_event_at().await?;

        Ok(IngestionAggregate {
            freshness: FreshnessStatus::from_last_event(last_event_at, Utc::now()),
            last_event_at,
        })
    }
}

#[cfg(test)]
#[path = "tests/ingestion_service_tests.rs"]
mod tests;
