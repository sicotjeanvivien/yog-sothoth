use std::net::SocketAddr;

use axum::{Router, http::HeaderName, routing::get};
use tower_http::{
    request_id::{PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::bootstrap::AppState;

mod cursor;
mod dto;
mod error;
mod handlers;
mod middleware;
mod query;

pub(crate) fn build_router(state: AppState) -> Router {
    let request_id_header = HeaderName::from_static(middleware::tracing::REQUEST_ID_HEADER);

    Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz))
        .route("/api/pools", get(handlers::pools::list_pools))
        .route("/api/pools/{address}", get(handlers::pools::get_pool))
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
        .route("/api/tokens/{mint}", get(handlers::token::get_token))
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(middleware::tracing::make_request_span)
                .on_request(middleware::tracing::on_request)
                .on_response(middleware::tracing::on_response)
                .on_failure(middleware::tracing::on_failure),
        )
        .layer(SetRequestIdLayer::new(
            request_id_header,
            middleware::tracing::GenerateRequestId,
        ))
        .layer(middleware::security_headers_layer())
        .layer(middleware::frame_options_layer())
        .layer(middleware::cors_layer())
}

pub(crate) async fn run(state: AppState, bind_addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind on {bind_addr}: {e}"))?;

    info!(addr = %bind_addr, "API server listening");

    let router = build_router(state);

    axum::serve(listener, router)
        .await
        .map_err(|e| anyhow::anyhow!("serve failed: {e}"))?;

    Ok(())
}
