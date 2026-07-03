//! Application service for the signal feed.
//!
//! Orchestrates pagination of the signals emitted by the yog-signals daemon
//! detectors. Pure domain: no axum, no DTOs, no HTTP concerns. The
//! handler is responsible for cursor wire encoding/decoding and DTO
//! mapping.

use std::sync::Arc;

use yog_core::{
    PageDirection, PagePosition, RepositoryError,
    domain::{Severity, SignalCursor, SignalFeed, SignalRecord},
    tools::Page,
};

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Input to [`SignalService::list_signals`].
pub(crate) struct SignalListParams {
    pub severity: Option<Severity>,
    pub cursor: Option<SignalCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for signal feed queries.
///
/// Depends on the feed lens only ([`SignalFeed`]) — the
/// engine's write/dedup contract never reaches the api process.
pub(crate) struct SignalService {
    repo: Arc<dyn SignalFeed>,
}

impl SignalService {
    pub(crate) fn new(repo: Arc<dyn SignalFeed>) -> Self {
        Self { repo }
    }

    /// Paginate the signal feed, optionally filtered to one severity.
    pub(crate) async fn list_signals(
        &self,
        params: SignalListParams,
    ) -> Result<Page<SignalRecord>, RepositoryError> {
        self.repo
            .list(
                params.severity,
                params.cursor,
                params.direction,
                params.position,
                params.limit,
            )
            .await
    }
}

#[cfg(test)]
#[path = "tests/signal_service_tests.rs"]
mod tests;
