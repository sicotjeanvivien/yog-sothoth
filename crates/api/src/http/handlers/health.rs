//! Probes for orchestration tooling (load balancer, Kubernetes,
//! Uptime Kuma, etc.).
//!
//! Two distinct concerns:
//!
//!   - `/healthz` — liveness. "Is the process alive?" Always 200
//!     while the binary runs. No external dependencies. If liveness
//!     fails, the orchestrator restarts the process; we want this
//!     to be a pure self-check, never coupled to a flaky upstream.
//!
//!   - `/readyz` — readiness. "Can the process serve a useful
//!     request right now?" Pings the database. If readiness fails,
//!     the load balancer drains traffic but does NOT restart — a
//!     restart wouldn't help if Postgres itself is the problem.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::bootstrap::AppState;
use crate::http::dto::response::{
    ChecksReport, ComponentStatus, ReadinessResponse, ReadinessStatus,
};

/// `GET /healthz` — liveness probe.
///
/// 200 OK with a static body. Does not touch the database.
pub(crate) async fn healthz() -> impl IntoResponse {
    "ok"
}

/// `GET /readyz` — readiness probe.
///
/// Returns 200 with `{ status: "ready", ... }` when every check
/// passes, 503 with `{ status: "unhealthy", checks: { db: "down" } }`
/// when at least one check fails. The 503 is what the load balancer
/// reads to drain traffic from this instance.
pub(crate) async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = state.health_checker.ping().await.is_ok();

    let body = ReadinessResponse {
        status: if db_ok {
            ReadinessStatus::Ready
        } else {
            ReadinessStatus::Unhealthy
        },
        checks: ChecksReport {
            db: if db_ok {
                ComponentStatus::Up
            } else {
                ComponentStatus::Down
            },
        },
    };

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}
