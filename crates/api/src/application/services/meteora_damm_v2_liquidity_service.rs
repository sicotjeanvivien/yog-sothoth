//! Application service for liquidity events.
//!
//! Mirrors `SwapService` exactly — same pagination contract,
//! different domain type.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    PageDirection, PagePosition, RepositoryError,
    domain::{
        MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventCursor,
        MeteoraDammV2LiquidityEventRepository,
    },
    tools::Page,
};

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Input to [`LiquidityService::list_liquidity_for_pool`].
pub(crate) struct MeteoraDammV2LiquidityListParams {
    pub pool_address: Pubkey,
    pub cursor: Option<MeteoraDammV2LiquidityEventCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for liquidity event queries.
pub(crate) struct MeteoraDammV2LiquidityService {
    repo: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
}

impl MeteoraDammV2LiquidityService {
    pub(crate) fn new(repo: Arc<dyn MeteoraDammV2LiquidityEventRepository>) -> Self {
        Self { repo }
    }

    /// Paginate liquidity events for a pool.
    pub(crate) async fn list_liquidity_for_pool(
        &self,
        params: MeteoraDammV2LiquidityListParams,
    ) -> Result<Page<MeteoraDammV2LiquidityEvent>, RepositoryError> {
        self.repo
            .find_by_pool_paginated(
                &params.pool_address,
                params.cursor,
                params.direction,
                params.position,
                params.limit,
            )
            .await
    }
}

#[cfg(test)]
#[path = "tests/liquidity_service_tests.rs"]
mod tests;
