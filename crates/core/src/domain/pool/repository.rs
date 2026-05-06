use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{RepositoryResult, domain::Pool};

/// Cursor identifying a position in the canonical pool ordering
/// (`first_seen_at DESC`, `pool_address ASC` as tiebreaker).
///
/// A cursor points to the *last item of the current page*; the next
/// page contains items strictly after this position in the ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCursor {
    pub first_seen_at: DateTime<Utc>,
    pub pool_address: Pubkey,
}

/// Persistence contract for Pool.
///
/// Implemented by the infrastructure layer (`yog-persistence`).
/// `core` defines the interface, consumers (indexer, api) wire the
/// concrete implementation under their own Postgres role.
///
/// At runtime, calls that exceed the role's grants will fail with a
/// permission error from Postgres — by design. The api role has only
/// `SELECT` on `pools`, so calling `upsert` from the api will fail.
#[async_trait]
pub trait PoolRepository: Send + Sync {
    // ---- Write-side (indexer) -------------------------------------------

    /// Insert a new pool, or refresh an existing one's `last_seen_at`.
    /// Used when an event arrives that fully describes the pool
    /// (Swap, Liquidity).
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()>;

    /// Refresh `last_seen_at` for an existing pool — but do NOT insert
    /// the row if the pool is unknown.
    ///
    /// Used by events that touch a pool without carrying enough info
    /// to populate it (ClaimPositionFee, ClaimReward — these don't
    /// expose the pool's mint addresses). If the pool isn't yet known,
    /// the call is a no-op; the next Swap or Liquidity event will
    /// create the row properly.
    async fn touch_last_seen(&self, pool_address: &Pubkey) -> RepositoryResult<()>;

    // ---- Read-side (api) ------------------------------------------------

    /// Fetch a single pool by its on-chain address.
    /// Returns `Ok(None)` if the pool has never been observed.
    async fn find_by_address(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<Pool>>;

    /// Paginate through known pools in canonical order:
    /// `first_seen_at DESC`, `pool_address ASC` as tiebreaker.
    ///
    /// `cursor` is `None` for the first page; for subsequent pages,
    /// pass the `next_cursor` returned by the previous call. `limit`
    /// is the maximum number of items to return; implementations
    /// may cap it to an upper bound.
    async fn find_paginated(
        &self,
        cursor: Option<PoolCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Pool>>;
}