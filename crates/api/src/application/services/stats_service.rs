//! Application service for protocol-wide statistics (`GET /api/stats`).
//!
//! Composes two reads — the USD aggregate analytics ([`GlobalAnalytics`]) and
//! the pool inventory counts ([`PoolCounts`]) — into a single aggregate. The
//! two concerns live on different repositories by design: counts are a `pools`
//! concern, USD valuation an analytics concern. This service only orchestrates;
//! no business logic, no SQL.

use std::sync::Arc;

use crate::application::WorkSlots;
use crate::application::cache::{SHARED_RESULT_TTL, TtlCache};
use crate::application::work_slots::no_work_slot;
use yog_core::{
    RepositoryError,
    domain::{GlobalAnalytics, GlobalAnalyticsRepository, PoolCatalog, PoolCounts},
};

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

/// The assembled protocol-wide statistics: USD analytics + pool counts.
/// `Clone`: one computation is shared by every caller of the cache.
#[derive(Debug, Clone)]
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
    pool_repo: Arc<dyn PoolCatalog>,
    /// `/api/stats` is the same for every visitor: computed once per
    /// [`SHARED_RESULT_TTL`], however many requests arrive together.
    cache: TtlCache<(), StatsAggregate, RepositoryError>,
    /// Taken while the stats compute, never by a caller waiting for them.
    work_slots: WorkSlots,
}

impl StatsService {
    pub(crate) fn new(
        global_analytics_repo: Arc<dyn GlobalAnalyticsRepository>,
        pool_repo: Arc<dyn PoolCatalog>,
        work_slots: WorkSlots,
    ) -> Self {
        Self {
            global_analytics_repo,
            pool_repo,
            cache: TtlCache::new(SHARED_RESULT_TTL),
            work_slots,
        }
    }

    /// Assemble the current protocol-wide statistics.
    pub(crate) async fn get_stats(&self) -> Result<StatsAggregate, RepositoryError> {
        self.cache
            .get_or_try_compute((), || async {
                let _slot = self.work_slots.acquire().await.ok_or_else(no_work_slot)?;
                self.compute_stats().await
            })
            .await
    }

    /// The two reads themselves; only [`Self::get_stats`] calls it, through
    /// the cache.
    async fn compute_stats(&self) -> Result<StatsAggregate, RepositoryError> {
        let analytics = self.global_analytics_repo.global_analytics().await?;
        let counts = self.pool_repo.counts().await?;

        Ok(StatsAggregate { analytics, counts })
    }
}

#[cfg(test)]
#[path = "tests/stats_service_tests.rs"]
mod tests;
