//! Capacity bounds: what keeps one client from taking the API down for
//! everyone.
//!
//! Measured on 25 September 2026, before these bounds existed, with the dev
//! database behind a 10-connection pool:
//!
//! - 30 parallel `/api/pools/top` from one client → 10 answered `500`, and
//!   `/api/tokens/{mint}` (4 ms at rest) answered `500` after 5.0 s — the
//!   slow routes (1.8–3.7 s each) held every connection;
//! - ~500 `/api/signals/stream` connections → the process OOM-killed under
//!   the production `mem_limit` of 32 MiB.
//!
//! Every number below is named once, here, with the measurement it answers.
//! The SSE cap lives beside the others but is applied by the stream handler,
//! which holds its permit for the life of the connection.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tokio::sync::Semaphore;

use crate::http::error::ApiError;

/// How long a request may take to produce its response.
///
/// Above [`STATEMENT_TIMEOUT`], so that a statement Postgres cancels surfaces
/// as the repository error it is, before this deadline cuts the request. It
/// bounds the response's *production*, not its body: the SSE stream, which
/// answers its headers at once, is not cut by it.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How long Postgres lets one of the API's statements run.
///
/// The slowest route measured at rest took 3.7 s; twice that leaves room for
/// a loaded database before a statement is abandoned. Only the server can
/// enforce it: a request cut client-side leaves its statement running.
pub(crate) const STATEMENT_TIMEOUT: Duration = Duration::from_secs(8);

/// How many slow requests may run at once.
///
/// Below the pool's 10 connections ([`yog_persistence::Database::DEFAULT_MAX_CONNECTIONS`]),
/// so the light routes and the signal poller always keep 4.
pub(crate) const HEAVY_ROUTE_PERMITS: usize = 6;

/// How long a slow request waits for a slot before being refused.
///
/// Long enough for the bursts a page makes on its own (a dashboard loads
/// three slow routes at once), short enough that a refused client learns so
/// quickly rather than piling up.
pub(crate) const HEAVY_ROUTE_WAIT: Duration = Duration::from_secs(2);

/// How many `/api/signals/stream` connections may be open at once.
///
/// A stream costs about 60 KiB (500 streams took the process from 7 to
/// 37 MiB): 200 of them fit in the production `mem_limit` of 32 MiB with the
/// process's own baseline.
pub(crate) const SSE_MAX_STREAMS: usize = 200;

/// Refuse a request that has not produced its response within `limit`.
pub(crate) async fn request_deadline(
    State(limit): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(limit, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::Unavailable("the request took too long; retry shortly").into_response(),
    }
}

/// The slots shared by every slow route, process-wide.
///
/// One semaphore behind an `Arc`, handed to the middleware of each slow
/// route: `tower::limit::ConcurrencyLimitLayer` would give each route its own
/// semaphore under axum, and refuse outside Problem Details.
#[derive(Clone)]
pub(crate) struct HeavyRouteLimit {
    slots: Arc<Semaphore>,
    wait: Duration,
}

impl HeavyRouteLimit {
    pub(crate) fn new(permits: usize, wait: Duration) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(permits)),
            wait,
        }
    }
}

/// Run a slow route only once it holds a slot; refuse it if none frees up
/// within the wait.
pub(crate) async fn heavy_route_limit(
    State(limit): State<HeavyRouteLimit>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(limit.wait, limit.slots.acquire_owned()).await {
        Ok(Ok(_slot)) => next.run(request).await,
        // Timed out, or the semaphore was closed (it never is).
        _ => ApiError::Unavailable("the API is at capacity; retry shortly").into_response(),
    }
}

#[cfg(test)]
#[path = "capacity_tests.rs"]
mod tests;
