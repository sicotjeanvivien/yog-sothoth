use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{RepositoryResult, domain::LiquidityEvent};

/// Cursor identifying a position in the canonical liquidity-event
/// ordering (`timestamp DESC`, `signature ASC` as tiebreaker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityCursor {
    pub timestamp: DateTime<Utc>,
    pub signature: String,
}

/// Persistence contract for liquidity events.
#[async_trait]
pub trait LiquidityEventRepository: Send + Sync {
    // ---- Write-side (indexer) -------------------------------------------

    async fn insert(&self, event: &LiquidityEvent) -> RepositoryResult<()>;

    // ---- Read-side (api) ------------------------------------------------

    /// Paginate liquidity events for a given pool, ordered by
    /// `timestamp DESC`, `signature ASC` as tiebreaker.
    ///
    /// `cursor` is `None` for the first page; for subsequent pages,
    /// pass the `next_cursor` returned by the previous call. `limit`
    /// is the maximum number of items to return; implementations
    /// may cap it to an upper bound.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<LiquidityCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityEvent>>;
}
