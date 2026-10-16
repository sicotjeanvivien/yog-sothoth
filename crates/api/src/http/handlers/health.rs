//! Probes. `/healthz` is liveness, a self-check with no dependency: failing it restarts the
//! process. `/readyz` is readiness and pings the database: failing it drains traffic, since
//! a restart would not fix Postgres.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::http::{
    AppState,
    dto::response::{
        ChecksReportResponse, ComponentStatusResponse, ReadinessResponse, ReadinessStatusResponse,
    },
};

/// `GET /healthz`: 200 with a static body, without touching the database.
pub(crate) async fn healthz() -> impl IntoResponse {
    "ok"
}

/// `GET /readyz`: 200 when every check passes, 503 with the failed checks otherwise.
pub(crate) async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = state.health_checker.ping().await.is_ok();

    let body = ReadinessResponse {
        status: if db_ok {
            ReadinessStatusResponse::Ready
        } else {
            ReadinessStatusResponse::Unhealthy
        },
        checks: ChecksReportResponse {
            db: if db_ok {
                ComponentStatusResponse::Up
            } else {
                ComponentStatusResponse::Down
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
