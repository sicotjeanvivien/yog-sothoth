//! HTTP layer powered by axum.
//!
//! Routes are mounted by `build_router`; the application state is
//! shared via axum's `State` extractor. Handlers live in `handlers/`,
//! the unified error type in `error.rs`.

mod dto;
mod error;
mod handlers;

use std::net::SocketAddr;

use axum::{Router, routing::get};
use tracing::info;

use crate::bootstrap::AppState;

/// Build the axum router from the application state.
pub(crate) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/api/pools", get(handlers::pools::list_pools))
        .with_state(state)
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
