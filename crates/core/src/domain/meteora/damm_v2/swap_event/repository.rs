use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::RepositoryResult;
use crate::domain::MeteoraDammV2SwapEvent;
use crate::tools::Page;
use crate::{PageDirection, PagePosition};

/// Cursor identifying a position in the canonical swap-event ordering
/// (`timestamp DESC`, `signature ASC` as tiebreaker).
///
/// A cursor points to the *last item of the current page*; the next
/// page contains items strictly after this position in the ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2SwapEventCursor {
    pub timestamp: DateTime<Utc>,
    pub signature: Signature,
}

/// Persistence contract for swap events — the write side, owned by the
/// indexer's persistor. The read side lives in
/// [`MeteoraDammV2SwapEventFeed`].
#[async_trait]
pub trait MeteoraDammV2SwapEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2SwapEvent) -> RepositoryResult<()>;
}

/// The per-pool swap-event feed — the api's lens: a cursor-paginated,
/// time-ordered listing, same shape as the signal feed (`SignalFeed`).
///
/// Kept separate from [`MeteoraDammV2SwapEventRepository`] (write side,
/// indexer) so each binary depends on exactly the methods it uses.
#[async_trait]
pub trait MeteoraDammV2SwapEventFeed: Send + Sync {
    /// Paginate swap events for a given pool, ordered by
    /// `timestamp DESC`, `signature ASC` as tiebreaker.
    ///
    /// `cursor` is `None` for the first page; for subsequent pages,
    /// pass the `next_cursor` returned by the previous call. `limit`
    /// is the maximum number of items to return; implementations
    /// may cap it to an upper bound.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<MeteoraDammV2SwapEventCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2SwapEvent>>;
}
