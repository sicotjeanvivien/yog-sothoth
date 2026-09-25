//! HTTP layer powered by axum.
//!
//! Routes are mounted by `build_router`; the application state is
//! shared via axum's `State` extractor. Handlers live in `handlers/`,
//! middleware in `middleware/`, the unified error type in `error.rs`.
//!
//! Probe endpoints (`/healthz`, `/readyz`) are mounted on a separate
//! sub-router that bypasses the tracing and request-id layers. Their
//! sole purpose is to answer load-balancer polling — quietly and
//! often — and routing them through tracing would flood the logs.

mod cursor;
mod dto;
mod error;
mod handlers;
mod middleware;
mod query;

/// The SSE response, mounted by the shutdown tests without the state it is
/// normally read from.
#[cfg(test)]
pub(crate) use handlers::signals::signal_sse;

/// The bounds the bootstrap applies outside the router: the statement limit
/// on the database connection, and the cap on open signal streams.
pub(crate) use middleware::capacity::{SSE_MAX_STREAMS, STATEMENT_TIMEOUT};

use std::net::SocketAddr;

use axum::{Router, http::HeaderValue, middleware::from_fn_with_state, routing::get};
use tower_http::{
    request_id::{PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::bootstrap::AppState;
use crate::http::middleware::capacity::{
    HEAVY_ROUTE_PERMITS, HEAVY_ROUTE_WAIT, HeavyRouteLimit, REQUEST_TIMEOUT, heavy_route_limit,
    request_deadline,
};
use crate::http::middleware::tracing::{
    GenerateRequestId, REQUEST_ID_HEADER, make_request_span, on_failure, on_request, on_response,
};

/// Build the axum router from the application state.
///
/// Two sub-routers are merged:
///
/// 1. `probes`  — `/healthz`, `/readyz`. No tracing, no request id.
///    Designed to be hit constantly by orchestration tooling without
///    leaving a trace in the logs.
/// 2. `app` — every business endpoint. Wrapped in `TraceLayer` for
///    per-request spans and in the request-id layers for correlation,
///    and bounded by [`REQUEST_TIMEOUT`]. Its slow routes form a
///    sub-router sharing [`HEAVY_ROUTE_PERMITS`] slots — see
///    [`middleware::capacity`] for the measurements behind each bound.
///
/// Cross-cutting headers (security, CORS, frame-options) apply to
/// both — they are hung on the merged router below.
pub(crate) fn build_router(state: AppState, cors_allowed_origins: Vec<HeaderValue>) -> Router {
    let probes = Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz));

    // ── Slow routes: behind the shared slots ────────────────────────────
    // The routes measured above 0.5 s at rest (25 September 2026). They
    // share `HEAVY_ROUTE_PERMITS` slots, below the pool's size, so that a
    // burst on them cannot take the connections the light routes need.
    let heavy = Router::new()
        // ── Pool collection ─────────────────────────────────────────────
        .route("/api/pools", get(handlers::pools::list_pools))
        // ── Ranked pools (non-paginated, capped) ─────────────────────────
        .route("/api/pools/top", get(handlers::pools::list_top_pools))
        // ── Single-pool resources ───────────────────────────────────────
        .route("/api/pools/{address}", get(handlers::pools::get_pool))
        .route(
            "/api/pools/{address}/history",
            get(handlers::pools::get_pool_history),
        )
        // ── Signal feed (paginated) ─────────────────────────────────────
        .route("/api/signals", get(handlers::signals::list_signals))
        .route("/api/stats", get(handlers::stats::get_stats))
        .route_layer(from_fn_with_state(
            HeavyRouteLimit::new(HEAVY_ROUTE_PERMITS, HEAVY_ROUTE_WAIT),
            heavy_route_limit,
        ));

    let app = Router::new()
        // ── Operator announcements (non-paginated, active window) ───────
        .route(
            "/api/announcements/active",
            get(handlers::announcements::list_active_announcements),
        )
        // ── Fee-tier option list (non-paginated) — powers the fee filter ──
        .route("/api/pools/fee-tiers", get(handlers::pools::list_fee_tiers))
        // ── Single-pool resources ───────────────────────────────────────
        .route(
            "/api/pools/{address}/latest-state",
            get(handlers::pools::get_pool_latest_state),
        )
        .route(
            "/api/pools/{address}/swap-events",
            get(handlers::pools::list_pool_swaps),
        )
        .route(
            "/api/pools/{address}/liquidity-events",
            get(handlers::pools::list_pool_liquidity_events),
        )
        .route(
            "/api/network/status",
            get(handlers::network_status::get_network_status),
        )
        // ── Signal feed (live) — capped by its own slots, see the handler ──
        .route(
            "/api/signals/stream",
            get(handlers::signals::stream_signals),
        )
        .route("/api/tokens/{mint}", get(handlers::token::get_token))
        .merge(heavy)
        // ── Deadline (innermost: the tracing span records its 503) ───────
        .layer(from_fn_with_state(REQUEST_TIMEOUT, request_deadline))
        // ── Tracing and request id (applied only here) ───────────────────
        // Inner-to-outer:
        //   1. PropagateRequestIdLayer echoes the id on the response.
        //   2. TraceLayer creates the per-request span.
        //   3. SetRequestIdLayer ensures the id is on the request
        //      before TraceLayer reads it.
        .layer(PropagateRequestIdLayer::new(
            axum::http::HeaderName::from_static(REQUEST_ID_HEADER),
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(make_request_span)
                .on_request(on_request)
                .on_response(on_response)
                .on_failure(on_failure),
        )
        .layer(SetRequestIdLayer::new(
            axum::http::HeaderName::from_static(REQUEST_ID_HEADER),
            GenerateRequestId,
        ));

    Router::new()
        .merge(probes)
        .merge(app)
        .with_state(state)
        // Security headers and CORS apply to everything, probes included.
        // No log noise concern — these layers don't emit logs.
        .layer(middleware::security_headers_layer())
        .layer(middleware::frame_options_layer())
        .layer(middleware::cors_layer(cors_allowed_origins))
}

/// Bind the listener the server will accept on.
///
/// Kept apart from serving so that the address is taken, or refused, before
/// anything is spawned: a port already in use is a startup error, not a task
/// that dies later.
pub(crate) async fn bind(bind_addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind on {bind_addr}: {e}"))?;

    info!(addr = %bind_addr, "API server listening");
    Ok(listener)
}
