use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{PageDirection, PagePosition, PoolSort, PoolSortColumn};
use crate::{
    RepositoryResult,
    domain::{Pool, PoolRegistryProperties},
};

/// Cursor in a pool ordering: the anchor row's sort value, and its address as
/// tiebreaker. Carries its sort column, so a cursor replayed under another is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCursor {
    pub sort_column: PoolSortColumn,
    pub sort_value: DateTime<Utc>,
    pub pool_address: Pubkey,
    /// Fence of the traversal, see [`PoolPage::as_of`]; when `None`,
    /// [`PoolCatalog::find_paginated`] mints a fresh one.
    pub as_of: Option<DateTime<Utc>>,
}

/// A page of the pool listing and the snapshot fence of its traversal.
/// ⚠️ `last_seen_at` moves under keyset pagination: reading only `<= as_of`
/// makes the set shrink-only, so a touched pool leaves instead of moving.
#[derive(Debug, Clone)]
pub struct PoolPage {
    pub page: Page<Pool>,
    /// The fence; `None` on a sort over an immutable column.
    /// ⚠️ The first page is served unfenced: bounding it could only drop rows.
    pub as_of: Option<DateTime<Utc>>,
    /// Pools matching the same filters that became active after `as_of`.
    /// ⚠️ An upper bound, never "pools you missed": it counts pools already shown,
    /// and misses one that enters a filter without being touched.
    pub touched_since: i64,
}

/// A paginated pool listing, fully validated by the HTTP layer: `position` jumps
/// to a boundary and excludes `cursor`; `direction` is ignored without a cursor.
/// `fee_bps` is one of [`PoolCatalog::list_fee_tiers`]; `limit` is clamped.
#[derive(Debug, Clone)]
pub struct PoolListQuery {
    pub cursor: Option<PoolCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub sort: PoolSort,
    pub search: Option<String>,
    pub fee_bps: Option<rust_decimal::Decimal>,
    pub limit: i64,
}

/// Every pool ever recorded, and how many were first seen in the last 24h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCounts {
    pub observed: i64,
    pub discovered_24h: i64,
}

/// One option of the fee filter: a base-fee tier in bps and how many pools carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeTier {
    pub fee_bps: rust_decimal::Decimal,
    pub pool_count: i64,
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
    /// A pool by address, `None` if never observed.
    async fn find_by_address(&self, pool_address: &Pubkey) -> RepositoryResult<Option<Pool>>;

    /// See [`PoolCounts`].
    async fn counts(&self) -> RepositoryResult<PoolCounts>;

    /// Many pools in one query, unordered, unknown addresses omitted.
    async fn find_by_addresses(&self, pool_addresses: &[Pubkey]) -> RepositoryResult<Vec<Pool>>;

    /// A page of pools; the fence is minted here when the cursor carries none,
    /// so a caller can neither forget it nor contradict the cursor.
    async fn find_paginated(&self, query: PoolListQuery) -> RepositoryResult<PoolPage>;

    /// The **most common** base-fee tiers, ascending by fee, unresolved pools excluded.
    async fn list_fee_tiers(&self) -> RepositoryResult<Vec<FeeTier>>;
}
