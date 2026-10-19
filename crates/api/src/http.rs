//! HTTP layer powered by axum: `build_router` mounts the routes. ⚠️ The probes (`/healthz`,
//! `/readyz`) bypass the tracing and request-id layers: load-balancer polling would flood the logs.

#[cfg(test)]
#[path = "http/tests/contract.rs"]
mod contract;
mod cursor;
mod dto;
mod error;
mod handlers;
mod middleware;
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

use axum::{
    Router,
    http::HeaderValue,
    middleware::{from_fn_with_state, map_response},
    routing::get,
};
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
        // Innermost, so the tracing span records its 503.
        .layer(from_fn_with_state(REQUEST_TIMEOUT, request_deadline))
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
        // ⚠️ Outermost: the id must be on the request before TraceLayer reads it.
        .layer(SetRequestIdLayer::new(
            axum::http::HeaderName::from_static(REQUEST_ID_HEADER),
            GenerateRequestId,
        ));

    Router::new()
        .merge(probes)
        .merge(app)
        .with_state(state)
        // Wraps the default fallback too: an unknown route is answered here.
        .layer(map_response(
            middleware::problem_details::answer_in_problem_details,
        ))
        // Security headers and CORS apply to everything, probes included.
        .layer(middleware::security_headers_layer())
        .layer(middleware::frame_options_layer())
        .layer(middleware::cors_layer(cors_allowed_origins))
}

/// Every `/api` route, each mounted with its contract (`#[utoipa::path]`). The
/// committed OpenAPI document is read from this same list, so a route cannot be
/// served without being declared.
fn api_routes(work_slots: WorkSlots) -> OpenApiRouter<AppState> {
    // ⚠️ `/api/stats` is slow too, but stays out: its computation takes a slot
    // inside the cache, so its waiters hold none.
    let slotted = OpenApiRouter::new()
        .routes(routes!(handlers::signals::list_signals))
        .route_layer(from_fn_with_state(work_slots, work_slot_limit));

    OpenApiRouter::new()
        .routes(routes!(handlers::announcements::list_active_announcements))
        .routes(routes!(handlers::stats::get_stats))
        .routes(routes!(handlers::network::get_network))
        .routes(routes!(handlers::ingestion::get_ingestion))
        // The live feed is capped by its own slots, in the handler.
        .routes(routes!(handlers::signals::stream_signals))
        .routes(routes!(handlers::token::get_token))
        .merge(slotted)
}

/// Binds the listener before anything is spawned: a port in use is a startup error, not a
/// task that dies later.
pub(crate) async fn bind(bind_addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind on {bind_addr}: {e}"))?;

    info!(addr = %bind_addr, "API server listening");
    Ok(listener)
}
