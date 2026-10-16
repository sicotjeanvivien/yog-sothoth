use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::RepositoryResult;
use crate::domain::{InsertOutcome, MeteoraDammV2SwapEvent};
use crate::tools::Page;
use crate::{PageDirection, PagePosition};

/// The last item of a page in the swap-event ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2SwapEventCursor {
    pub timestamp: DateTime<Utc>,
    pub signature: Signature,
    /// ⚠️ Third key of the ordering: the hops of a routed transaction share
    /// `timestamp` and `signature`, and a page boundary between them drops one.
    pub event_index: u16,
}

/// Write side of swap events, the indexer's lens.
#[async_trait]
pub trait MeteoraDammV2SwapEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2SwapEvent) -> RepositoryResult<InsertOutcome>;
}

/// The per-pool swap-event feed, the api's lens.
#[async_trait]
pub trait MeteoraDammV2SwapEventFeed: Send + Sync {
    /// A page of the pool's events, `(timestamp DESC, signature ASC, event_index ASC)`;
    /// the returned cursors chain the pages, `limit` may be capped.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<MeteoraDammV2SwapEventCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2SwapEvent>>;
}
