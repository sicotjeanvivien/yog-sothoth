//! Application service for swap events.
//!
//! Orchestrates pagination of swap events for a given pool. Pure
//! domain: no axum, no DTOs, no HTTP concerns. The handler is
//! responsible for cursor wire encoding/decoding and DTO mapping.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    PageDirection, PagePosition, RepositoryError,
    domain::{SwapCursor, SwapEvent, SwapEventRepository},
    tools::Page,
};

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Input to [`SwapService::list_swaps_for_pool`].
pub(crate) struct SwapListParams {
    pub pool_address: Pubkey,
    pub cursor: Option<SwapCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for swap event queries.
pub(crate) struct SwapService {
    repo: Arc<dyn SwapEventRepository>,
}

impl SwapService {
    pub(crate) fn new(repo: Arc<dyn SwapEventRepository>) -> Self {
        Self { repo }
    }

    /// Paginate swap events for a pool.
    pub(crate) async fn list_swaps_for_pool(
        &self,
        params: SwapListParams,
    ) -> Result<Page<SwapEvent>, RepositoryError> {
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
