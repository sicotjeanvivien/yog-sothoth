use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{
    Page, RepositoryResult,
    domain::{Pool, PoolCursor, PoolEntry, PoolRegistryProperties, PoolScope, Protocol},
};

/// How many pools of one protocol were ever recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolPools {
    pub protocol: Protocol,
    pub pools_seen: i64,
}

/// Write side of the pool registry, the indexer's lens; [`PoolCatalog`] reads it.
#[async_trait]
pub trait PoolRepository: Send + Sync {
    /// Insert a new pool, or refresh an existing one's `last_seen_at`.
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()>;

    /// Refresh `last_seen_at` of a known pool; a no-op for an unknown one.
    async fn touch_last_seen(&self, pool_address: &Pubkey) -> RepositoryResult<()>;

    /// Flag a pool's account-derived properties for yog-context to re-read;
    /// idempotent, a no-op for an unknown pool.
    /// ⚠️ The indexer's only way to affect a property: never write the value itself.
    async fn mark_needs_refresh(&self, pool_address: &Pubkey) -> RepositoryResult<()>;

    /// Store the token pair and base fee decoded from the account, and clear the flag.
    /// ⚠️ Call it after the protocol satellite: it lowers the refresh flag, so a
    /// failure before it leaves the pool queued.
    async fn set_registry_properties(
        &self,
        pool_address: &Pubkey,
        core: &PoolRegistryProperties,
    ) -> RepositoryResult<()>;
}

/// Read side of the pool registry, the api's lens.
#[async_trait]
pub trait PoolCatalog: Send + Sync {
    /// One row per protocol with at least one pool recorded.
    async fn counts_by_protocol(&self) -> RepositoryResult<Vec<ProtocolPools>>;

    /// Many pools in one query, unordered, unknown addresses omitted.
    async fn find_by_addresses(&self, pool_addresses: &[Pubkey]) -> RepositoryResult<Vec<Pool>>;

    /// A page of `scope`'s pools, the most recently first seen first, ties by address
    /// descending, after `cursor`'s key; forward only. `cursor` is of `scope`.
    async fn list_entries(
        &self,
        scope: PoolScope,
        cursor: Option<PoolCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolEntry>>;

    /// The pool at `pool_address`, `None` if never recorded.
    async fn find_entry(&self, pool_address: &Pubkey) -> RepositoryResult<Option<PoolEntry>>;
}
