use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::tools::Page;
use crate::{PageDirection, PagePosition};
use crate::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventValued},
};

/// The last item of a page in the liquidity-event ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2LiquidityEventCursor {
    pub timestamp: DateTime<Utc>,
    pub signature: Signature,
    /// ⚠️ Third key of the ordering: the hops of a routed transaction share
    /// `timestamp` and `signature`, and a page boundary between them drops one.
    pub event_index: u16,
}

/// Write side of liquidity events, the indexer's lens.
#[async_trait]
pub trait MeteoraDammV2LiquidityEventRepository: Send + Sync {
    async fn insert(&self, event: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<InsertOutcome>;
}

/// The per-pool liquidity-event feed, the api's lens.
#[async_trait]
pub trait MeteoraDammV2LiquidityEventFeed: Send + Sync {
    /// A page of the pool's events, `(timestamp DESC, signature ASC, event_index ASC)`.
    /// Each carries its USD value at event time, see [`MeteoraDammV2LiquidityEventValued`].
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<MeteoraDammV2LiquidityEventCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2LiquidityEventValued>>;
}
