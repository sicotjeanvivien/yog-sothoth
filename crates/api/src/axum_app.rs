//! Axum stack — transitional module during the migration away from
//! the custom HTTP layer in `interface/http/`.
//!
//! For commit 1, this module exposes a single `/healthz` endpoint on a
//! separate port (`AXUM_BIND_TRANSITIONAL`) so we can validate that
//! axum compiles, integrates with our `AppState`, and serves traffic —
//! all without touching the production endpoint on the main port.
//!
//! The custom stack on the main port is unchanged. This whole module
//! grows over the next commits and eventually replaces the custom
//! stack entirely.

use axum::{Router, response::IntoResponse, routing::get};
use tracing::info;

use crate::bootstrap::AppState;

/// Bind address used during the migration. Hard-coded so we don't
/// pollute the production env vars with a variable that will be
/// deleted in commit 3.
const AXUM_BIND_TRANSITIONAL: &str = "127.0.0.1:3001";

/// Build the axum router from the application state.
///
/// Will grow as endpoints are migrated; for commit 1 it only carries
/// `/healthz`.
pub(crate) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .with_state(state)
}

/// Liveness probe. Returns 200 OK with a static body. Does NOT touch
/// the database — that is what readiness probes are for, and we'll
/// add one when needed.
async fn healthz() -> impl IntoResponse {
    "ok"
}

/// Run the axum server until the process is killed.
///
/// Spawned as a separate task in `main` for the duration of the
/// migration, alongside the custom HTTP server. When commit 3 retires
/// the custom stack, this function becomes the single entry point.
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
