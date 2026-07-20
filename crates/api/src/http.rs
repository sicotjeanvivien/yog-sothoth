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

use std::net::SocketAddr;

use axum::{Router, http::HeaderValue, routing::get};
use tower_http::{
    request_id::{PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::bootstrap::AppState;
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
///    per-request spans and in the request-id layers for correlation.
///
/// Cross-cutting headers (security, CORS, frame-options) apply to
/// both — they are hung on the merged router below.
pub(crate) fn build_router(state: AppState, cors_allowed_origins: Vec<HeaderValue>) -> Router {
    let probes = Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz));

    let app = Router::new()
        // ── Operator announcements (non-paginated, active window) ───────
        .route(
            "/api/announcements/active",
            get(handlers::announcements::list_active_announcements),
        )
        // ── Pool collection ─────────────────────────────────────────────
        .route("/api/pools", get(handlers::pools::list_pools))
        // ── Ranked pools (non-paginated, capped) ─────────────────────────
        .route("/api/pools/top", get(handlers::pools::list_top_pools))
        // ── Single-pool resources ───────────────────────────────────────
        .route("/api/pools/{address}", get(handlers::pools::get_pool))
        .route(
            "/api/pools/{address}/latest-state",
            get(handlers::pools::get_pool_latest_state),
        )
        .route(
            "/api/pools/{address}/history",
            get(handlers::pools::get_pool_history),
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
        // ── Signal feed ─────────────────────────────────────────────────
        .route("/api/signals", get(handlers::signals::list_signals))
        .route(
            "/api/signals/stream",
            get(handlers::signals::stream_signals),
        )
        .route("/api/stats", get(handlers::stats::get_stats))
        .route("/api/tokens/{mint}", get(handlers::token::get_token))
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

/// Run the axum server on `bind_addr` until the process is killed.
pub(crate) async fn run(
    state: AppState,
    bind_addr: SocketAddr,
    cors_allowed_origins: Vec<HeaderValue>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind on {bind_addr}: {e}"))?;

    info!(addr = %bind_addr, "API server listening");

    let router = build_router(state, cors_allowed_origins);

    axum::serve(listener, router)
        .await
        .map_err(|e| anyhow::anyhow!("serve failed: {e}"))?;

    Ok(())
}
