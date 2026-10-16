//! When the most recent event was indexed: a repository of its own, it belongs to no table.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::RepositoryResult;

/// Reads the ingestion freshness signal.
#[async_trait]
pub trait EventFreshnessRepository: Send + Sync {
    /// The latest `timestamp` across `swap_events` and `liquidity_events`.
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>>;
}
