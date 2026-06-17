use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{PageDirection, PagePosition, PoolSort, PoolSortColumn};
use crate::{RepositoryResult, domain::Pool};

/// Cursor identifying a position in a pool ordering.
///
/// Carries the sort column it was built for, so the API layer can
/// reject a cursor that is replayed under a different sort (a
/// tampered or stale URL) rather than silently producing an
/// inconsistent page.
///
/// `sort_value` is the value of the active sort column for the anchor
/// row; `pool_address` is the unique tiebreaker. Both `first_seen_at`
/// and `last_seen_at` are `TIMESTAMPTZ`, so a single `DateTime<Utc>`
/// covers every supported column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCursor {
    pub sort_column: PoolSortColumn,
    pub sort_value: DateTime<Utc>,
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

    /// Set the pool's base trading fee (basis points), decoded from its
    /// genesis fee config. A column-level `UPDATE`; a no-op if the pool row
    /// does not exist yet (the caller registers it first). Idempotent.
    async fn set_fee_bps(
        &self,
        pool_address: &Pubkey,
        fee_bps: rust_decimal::Decimal,
    ) -> RepositoryResult<()>;

    // ---- Read-side (api) ------------------------------------------------

    /// Fetch a single pool by its on-chain address.
    /// Returns `Ok(None)` if the pool has never been observed.
    async fn find_by_address(&self, pool_address: &Pubkey) -> RepositoryResult<Option<Pool>>;

    /// Fetch a page of pools.
    ///
    /// - `cursor` + `direction` cooperate: traverse forward (`Next`)
    ///   or backward (`Prev`) from the cursor's position. Without a
    ///   cursor, `direction` is ignored and the natural ordering is
    ///   used (newest first).
    /// - `position` jumps to a list boundary (`First` or `Last`),
    ///   ignoring any cursor. Mutually exclusive with `cursor`.
    /// - `PoolSort`
    /// - `search`
    /// - `limit` is the maximum number of items returned; the
    ///   repository clamps it defensively.
    ///
    /// The returned `Page<Pool>` carries enough information for the
    /// caller to render Previous / Next / First / Last navigation
    /// without follow-up queries.
    async fn find_paginated(
        &self,
        cursor: Option<PoolCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        sort: PoolSort,
        search: Option<String>,
        limit: i64,
    ) -> RepositoryResult<Page<Pool>>;
}

/// Resolution of a pool's account-derived properties (token mints, base fee
/// and fee-split percents) from its on-chain cp-amm `Pool` account, performed
/// by yog-context
/// (which holds column-level UPDATE on those columns).
///
/// These properties can't be inferred reliably from the event stream: the
/// mints were mis-resolved by a per-event heuristic, and the base fee is only
/// emitted at pool genesis (`InitializePool`) — which the indexer never sees
/// for pools created before it started watching. Reading the account back-fills
/// both for every pool, old or new.
///
/// Kept separate from [`PoolRepository`] so the resolver worker depends
/// only on what it uses, and the read/write mocks in the api and
/// indexer crates don't have to carry these methods.
#[async_trait]
pub trait PoolAccountResolver: Send + Sync {
    /// Pools missing at least one account-derived property — a `NULL` mint, a
    /// `NULL` `fee_bps`, or a `NULL` fee-split percent — capped at `limit`.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>>;

    /// Set a pool's mints, base fee (basis points) and fee-split percents, as
    /// decoded from its on-chain account. A single column-level UPDATE;
    /// idempotent.
    // A flat list of resolved account columns — a wrapper struct would add
    // ceremony without clarifying anything.
    #[allow(clippy::too_many_arguments)]
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        token_a_mint: &Pubkey,
        token_b_mint: &Pubkey,
        fee_bps: rust_decimal::Decimal,
        protocol_fee_percent: u8,
        partner_fee_percent: u8,
        referral_fee_percent: u8,
    ) -> RepositoryResult<()>;
}
