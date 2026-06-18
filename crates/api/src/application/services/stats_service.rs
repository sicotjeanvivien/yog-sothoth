//! Application service for protocol-wide statistics (`GET /api/stats`).
//!
//! Composes two reads — the USD aggregate analytics ([`GlobalAnalytics`]) and
//! the pool inventory counts ([`PoolCounts`]) — into a single aggregate. The
//! two concerns live on different repositories by design: counts are a `pools`
//! concern, USD valuation an analytics concern. This service only orchestrates;
//! no business logic, no SQL.

use std::sync::Arc;

use yog_core::{
    RepositoryError,
    domain::{GlobalAnalytics, GlobalAnalyticsRepository, PoolCounts, PoolRepository},
};

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

/// The assembled protocol-wide statistics: USD analytics + pool counts.
#[derive(Debug)]
pub(crate) struct StatsAggregate {
    pub analytics: GlobalAnalytics,
    pub counts: PoolCounts,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for the global stats query.
pub(crate) struct StatsService {
    global_analytics_repo: Arc<dyn GlobalAnalyticsRepository>,
    pool_repo: Arc<dyn PoolRepository>,
}

impl StatsService {
    pub(crate) fn new(
        global_analytics_repo: Arc<dyn GlobalAnalyticsRepository>,
        pool_repo: Arc<dyn PoolRepository>,
    ) -> Self {
        Self {
            global_analytics_repo,
            pool_repo,
        }
    }

    /// Assemble the current protocol-wide statistics.
    pub(crate) async fn get_stats(&self) -> Result<StatsAggregate, RepositoryError> {
        let analytics = self.global_analytics_repo.global_analytics().await?;
        let counts = self.pool_repo.counts().await?;

        Ok(StatsAggregate { analytics, counts })
    }
}

#[cfg(test)]
#[path = "tests/stats_service_tests.rs"]
mod tests;
