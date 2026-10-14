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

#[cfg(test)]
mod contract;
mod cursor;
mod dto;
mod error;
mod handlers;
mod middleware;
mod query;
mod state;

pub(crate) use state::AppState;

/// The SSE response, mounted by the shutdown tests without the state it is
/// normally read from.
#[cfg(test)]
pub(crate) use handlers::signals::signal_sse;

/// The bounds the bootstrap applies outside the router: the statement limit
/// on the database connection, the cap on open signal streams, and the cap on
/// connections.
pub(crate) use middleware::capacity::{
    CappedListener, IDLE_TIMEOUT, MAX_CONNECTIONS, SSE_MAX_STREAMS, STATEMENT_TIMEOUT,
    WORK_SLOT_WAIT, WORK_SLOTS,
};

use std::net::SocketAddr;

use axum::{Router, http::HeaderValue, middleware::from_fn_with_state, routing::get};
use tower_http::{
    request_id::{PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::application::WorkSlots;

use crate::http::middleware::capacity::{REQUEST_TIMEOUT, request_deadline, work_slot_limit};
use crate::http::middleware::tracing::{
    GenerateRequestId, REQUEST_ID_HEADER, make_request_span, on_failure, on_request, on_response,
};

/// The probes (`/healthz`, `/readyz`, untraced) merged with [`api_routes`],
/// which is traced, request-id'd and bounded by [`REQUEST_TIMEOUT`]; both get
/// the security and CORS headers. The bounds are in [`middleware::capacity`].
pub(crate) fn build_router(state: AppState, cors_allowed_origins: Vec<HeaderValue>) -> Router {
    let probes = Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz));

    let (api, _contract) = api_routes(state.work_slots.clone()).split_for_parts();

    let app = api
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

/// Every `/api` route, each mounted with its contract (`#[utoipa::path]`). The
/// committed OpenAPI document is read from this same list, so a route cannot be
/// served without being declared.
fn api_routes(work_slots: WorkSlots) -> OpenApiRouter<AppState> {
    // ⚠️ `/api/pools/top` and `/api/stats` are slow too, but stay out: their
    // computation takes a slot inside the cache, so its waiters hold none.
    let slotted = OpenApiRouter::new()
        // ── Pool collection ─────────────────────────────────────────────
        .routes(routes!(handlers::pools::list_pools))
        // ── Single-pool resources ───────────────────────────────────────
        .routes(routes!(handlers::pools::get_pool))
        .routes(routes!(handlers::pools::get_pool_history))
        // ── Signal feed (paginated) ─────────────────────────────────────
        .routes(routes!(handlers::signals::list_signals))
        .route_layer(from_fn_with_state(work_slots, work_slot_limit));

    OpenApiRouter::new()
        // ── Operator announcements (non-paginated, active window) ───────
        .routes(routes!(handlers::announcements::list_active_announcements))
        // ── Shared results (cached, see the note on `slotted` above) ──────
        .routes(routes!(handlers::pools::list_top_pools))
        .routes(routes!(handlers::stats::get_stats))
        // ── Fee-tier option list (non-paginated) — powers the fee filter ──
        .routes(routes!(handlers::pools::list_fee_tiers))
        // ── Single-pool resources ───────────────────────────────────────
        .routes(routes!(handlers::pools::get_pool_latest_state))
        .routes(routes!(handlers::pools::list_pool_swaps))
        .routes(routes!(handlers::pools::list_pool_liquidity_events))
        .routes(routes!(handlers::network_status::get_network_status))
        // ── Signal feed (live) — capped by its own slots, see the handler ──
        .routes(routes!(handlers::signals::stream_signals))
        .routes(routes!(handlers::token::get_token))
        .merge(slotted)
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
