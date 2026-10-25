//! The pools the catalog knows: looked up by address, listed by scope, and read whole with
//! their protocol's properties.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        MeteoraDammV2PoolProperties, MeteoraDammV2PoolPropertiesLookup, Pool, PoolCatalog,
        PoolCursor, PoolEntry, PoolScope, Protocol,
    },
};

/// The properties of a pool's own protocol.
#[derive(Debug, Clone)]
pub(crate) enum PoolProperties {
    MeteoraDammV2(MeteoraDammV2PoolProperties),
}

/// A pool read whole: its entry, its protocol's properties, and the instant they were read
/// at, which the fees a curve charges now are evaluated at.
#[derive(Debug, Clone)]
pub(crate) struct PoolDetail {
    pub(crate) entry: PoolEntry,
    pub(crate) properties: Option<PoolProperties>,
    pub(crate) read_at: DateTime<Utc>,
}

pub(crate) struct PoolService {
    pools: Arc<dyn PoolCatalog>,
    damm_v2_properties: Arc<dyn MeteoraDammV2PoolPropertiesLookup>,
}

impl PoolService {
    pub(crate) fn new(
        pools: Arc<dyn PoolCatalog>,
        damm_v2_properties: Arc<dyn MeteoraDammV2PoolPropertiesLookup>,
    ) -> Self {
        Self {
            pools,
            damm_v2_properties,
        }
    }

    /// The pool at `address`, `None` when the catalog does not know it.
    pub(crate) async fn find_pool(&self, address: Pubkey) -> RepositoryResult<Option<Pool>> {
        Ok(self
            .pools
            .find_by_addresses(&[address])
            .await?
            .into_iter()
            .find(|pool| pool.pool_address == address))
    }

    pub(crate) async fn list_pools(
        &self,
        scope: PoolScope,
        cursor: Option<PoolCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolEntry>> {
        self.pools.list_entries(scope, cursor, limit).await
    }

    /// `None` when the catalog does not know `address`.
    pub(crate) async fn get_pool(&self, address: Pubkey) -> RepositoryResult<Option<PoolDetail>> {
        let Some(entry) = self.pools.find_entry(&address).await? else {
            return Ok(None);
        };
        // ⚠️ Exhaustive on purpose: a new protocol stops compilation here; a wildcard drops it.
        let properties = match entry.pool.protocol {
            Protocol::MeteoraDammV2 => self
                .damm_v2_properties
                .find_by_pool(&address)
                .await?
                .map(PoolProperties::MeteoraDammV2),
            Protocol::MeteoraDlmm => None,
        };
        Ok(Some(PoolDetail {
            entry,
            properties,
            read_at: Utc::now(),
        }))
    }
}

#[cfg(test)]
#[path = "tests/pool_service_tests.rs"]
mod tests;
