//! Application service for swap events.
//!
//! Orchestrates pagination of swap events for a given pool. Pure
//! domain: no axum, no DTOs, no HTTP concerns. The handler is
//! responsible for cursor wire encoding/decoding and DTO mapping.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    PageDirection, PagePosition, RepositoryError,
    domain::{
        MeteoraDammV2SwapEvent, MeteoraDammV2SwapEventCursor, MeteoraDammV2SwapEventRepository,
    },
    tools::Page,
};

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Input to [`SwapService::list_swaps_for_pool`].
pub(crate) struct MeteoraDammV2SwapListParams {
    pub pool_address: Pubkey,
    pub cursor: Option<MeteoraDammV2SwapEventCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for swap event queries.
pub(crate) struct MeteoraDammV2SwapService {
    repo: Arc<dyn MeteoraDammV2SwapEventRepository>,
}

impl MeteoraDammV2SwapService {
    pub(crate) fn new(repo: Arc<dyn MeteoraDammV2SwapEventRepository>) -> Self {
        Self { repo }
    }

    /// Paginate swap events for a pool.
    pub(crate) async fn list_swaps_for_pool(
        &self,
        params: MeteoraDammV2SwapListParams,
    ) -> Result<Page<MeteoraDammV2SwapEvent>, RepositoryError> {
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
#[path = "tests/swap_service_tests.rs"]
mod tests;
