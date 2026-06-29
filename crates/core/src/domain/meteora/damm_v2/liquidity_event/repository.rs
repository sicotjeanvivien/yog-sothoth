use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::tools::Page;
use crate::{PageDirection, PagePosition};
use crate::{
    RepositoryResult,
    domain::{MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventValued},
};

/// Cursor identifying a position in the canonical liquidity-event
/// ordering (`timestamp DESC`, `signature ASC` as tiebreaker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2LiquidityEventCursor {
    pub timestamp: DateTime<Utc>,
    pub signature: Signature,
}

/// Persistence contract for liquidity events.
#[async_trait]
pub trait MeteoraDammV2LiquidityEventRepository: Send + Sync {
    // ---- Write-side (indexer) -------------------------------------------

    async fn insert(&self, event: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<()>;

    // ---- Read-side (api) ------------------------------------------------

    /// Paginate liquidity events for a given pool, ordered by
    /// `timestamp DESC`, `signature ASC` as tiebreaker. Each item carries its
    /// trade-time USD value (`None` when not computable) — see
    /// [`MeteoraDammV2LiquidityEventValued`].
    ///
    /// `cursor` is `None` for the first page; for subsequent pages,
    /// pass the `next_cursor` returned by the previous call. `limit`
    /// is the maximum number of items to return; implementations
    /// may cap it to an upper bound.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<MeteoraDammV2LiquidityEventCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2LiquidityEventValued>>;
}
