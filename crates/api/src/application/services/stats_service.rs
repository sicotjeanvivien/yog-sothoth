//! Protocol-wide statistics (`GET /api/stats`): the USD analytics and the pool counts, two
//! repositories composed into one aggregate.

use std::sync::Arc;

use crate::application::WorkSlots;
use crate::application::cache::{STATS, SharedCache};
use crate::application::work_slots::no_work_slot;
use yog_core::{
    RepositoryError,
    domain::{GlobalAnalytics, GlobalAnalyticsRepository, PoolCatalog, PoolCounts},
};

/// The assembled protocol-wide statistics: USD analytics + pool counts.
/// `Clone`: one computation is shared by every caller of the cache.
#[derive(Debug, Clone)]
pub(crate) struct StatsAggregate {
    pub analytics: GlobalAnalytics,
    pub counts: PoolCounts,
}

pub(crate) struct StatsService {
    global_analytics_repo: Arc<dyn GlobalAnalyticsRepository>,
    pool_repo: Arc<dyn PoolCatalog>,
    /// `/api/stats`, computed once per [`STATS`] time to live for every visitor.
    cache: SharedCache<(), StatsAggregate>,
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
            cache: SharedCache::new(STATS),
            work_slots,
        }
    }

    pub(crate) async fn get_stats(&self) -> Result<StatsAggregate, RepositoryError> {
        self.cache
            .get_or_compute((), async {
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
