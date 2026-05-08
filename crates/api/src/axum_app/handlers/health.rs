use axum::response::IntoResponse;

/// Liveness probe. Returns 200 OK with a static body. Does NOT touch
/// the database — that is what readiness probes are for, and we'll
/// add one when needed.
pub(crate) async fn healthz() -> impl IntoResponse {
    "ok"
}
