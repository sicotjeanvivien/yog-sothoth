//! The protocol catalogue (`GET /api/protocols`): every protocol of the domain,
//! whether this build observes it, and how many of its pools were seen.

use std::sync::Arc;

use crate::application::WorkSlots;
use crate::application::cache::{PROTOCOLS, SharedCache};
use crate::application::work_slots::no_work_slot;
use yog_core::{
    RepositoryResult,
    domain::{PoolCatalog, Protocol},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtocolEntry {
    pub(crate) protocol: Protocol,
    pub(crate) observed: bool,
    pub(crate) pools_seen: i64,
}

pub(crate) struct ProtocolCatalogService {
    pool_repo: Arc<dyn PoolCatalog>,
    /// What the ingestion of this build subscribes to.
    observed: Vec<Protocol>,
    /// The catalogue, computed once per [`PROTOCOLS`] time to live for every visitor.
    cache: SharedCache<(), Vec<ProtocolEntry>>,
    work_slots: WorkSlots,
}

impl ProtocolCatalogService {
    pub(crate) fn new(
        pool_repo: Arc<dyn PoolCatalog>,
        observed: Vec<Protocol>,
        work_slots: WorkSlots,
    ) -> Self {
        Self {
            pool_repo,
            observed,
            cache: SharedCache::new(PROTOCOLS),
            work_slots,
        }
    }

    /// One entry per [`Protocol::all`], in its order; a protocol without a pool seen has 0.
    pub(crate) async fn list(&self) -> RepositoryResult<Vec<ProtocolEntry>> {
        self.cache
            .get_or_compute((), async {
                let _slot = self.work_slots.acquire().await.ok_or_else(no_work_slot)?;
                self.compute().await
            })
            .await
    }

    /// `protocol`'s entry of [`Self::list`].
    pub(crate) async fn get_protocol(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<ProtocolEntry>> {
        Ok(self
            .list()
            .await?
            .into_iter()
            .find(|entry| entry.protocol == protocol))
    }

    async fn compute(&self) -> RepositoryResult<Vec<ProtocolEntry>> {
        let counts = self.pool_repo.counts_by_protocol().await?;
        Ok(Protocol::all()
            .iter()
            .map(|&protocol| ProtocolEntry {
                protocol,
                observed: self.observed.contains(&protocol),
                pools_seen: counts
                    .iter()
                    .find(|count| count.protocol == protocol)
                    .map_or(0, |count| count.pools_seen),
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "tests/protocol_catalog_service_tests.rs"]
mod tests;
