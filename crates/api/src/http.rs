//! HTTP layer powered by axum.
//!
//! Routes are mounted by `build_router`; the application state is
//! shared via axum's `State` extractor. Handlers live in `handlers/`,
//! middleware in `middleware.rs`, the unified error type in `error.rs`.

mod dto;
mod error;
mod handlers;
mod middleware;

use std::net::SocketAddr;

use axum::{Router, routing::get};
use tracing::info;

use crate::bootstrap::AppState;

/// Build the axum router from the application state.
pub(crate) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::health::healthz))
        // ── Pool collection ─────────────────────────────────────────────
        .route("/api/pools", get(handlers::pools::list_pools))
        // ── Single-pool resources ───────────────────────────────────────
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
            axum::routing::get(crate::http::handlers::network_status::get_network_status),
        )
        .route(
            "/api/tokens/{mint}",
            axum::routing::get(crate::http::handlers::token::get_token),
        )
        .with_state(state)
        // Layers are applied in the order they are added. The
        // outermost layer (last added) sees requests first and
        // responses last. For headers + CORS, the order is
        // immaterial; documented for future contributors.
        .layer(middleware::security_headers_layer())
        .layer(middleware::frame_options_layer())
        .layer(middleware::cors_layer())
}

/// Run the axum server on `bind_addr` until the process is killed.
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
