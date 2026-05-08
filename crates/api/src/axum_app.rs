//! Axum stack — transitional module during the migration away from
//! the custom HTTP layer in `interface/http/`.
//!
//! Endpoints are migrated one at a time. Each commit moves one
//! endpoint from the custom stack (port 3000) to the axum stack
//! (port 3001) and validates by side-by-side comparison. When all
//! endpoints are migrated, axum is promoted to port 3000 and the
//! custom stack is removed.

mod error;
mod handlers;

use axum::{Router, routing::get};
use tracing::info;

use crate::bootstrap::AppState;

/// Bind address used during the migration. Hard-coded so we don't
/// pollute the production env vars with a variable that will be
/// deleted in commit 3.
const AXUM_BIND_TRANSITIONAL: &str = "127.0.0.1:3001";

/// Build the axum router from the application state.
pub(crate) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/api/pools", get(handlers::pools::list_pools))
        .with_state(state)
}

/// Run the axum server until the process is killed.
pub(crate) async fn run(state: AppState) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(AXUM_BIND_TRANSITIONAL)
        .await
        .map_err(|e| anyhow::anyhow!("axum: failed to bind on {AXUM_BIND_TRANSITIONAL}: {e}"))?;

    info!(
        addr = AXUM_BIND_TRANSITIONAL,
        "axum server listening (transitional)"
    );

    let router = build_router(state);

    axum::serve(listener, router)
        .await
        .map_err(|e| anyhow::anyhow!("axum: serve failed: {e}"))?;

    Ok(())
}
