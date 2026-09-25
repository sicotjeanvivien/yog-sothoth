use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tracing::{error, warn};

use crate::http::dto::response::{PROBLEM_CONTENT_TYPE, ProblemDetails};

/// API-level errors surfaced by handlers.
///
/// Four variants cover everything we need at this layer:
/// - `BadRequest`: client supplied invalid input (bad cursor, out-of-range limit).
/// - `NotFound`: resource does not exist.
/// - `Unavailable`: the API is over capacity or out of time — a slot, a
///   connection or a statement's budget ran out. Nothing is broken, and the
///   same request may succeed shortly: `503` with `Retry-After`, not `500`.
/// - `Internal`: unexpected failure — DB error, encoding bug, anything not the
///   client's fault. The detailed message is logged, never sent to the client.
///
/// Responses are serialised as RFC 9457 Problem Details with
/// `Content-Type: application/problem+json`. See
/// [`crate::http::dto::response::problem`] for the format rationale.
#[derive(Debug)]
pub(crate) enum ApiError {
    BadRequest(String),
    #[allow(dead_code)] // unused outside the pool/token handlers
    NotFound(String),
    Unavailable(&'static str),
    Internal(String),
}

/// What `Retry-After` tells a refused client, in seconds: about the time a
/// slot takes to free up, the slow routes answering in 2–4 s (measured
/// 25 September 2026).
const RETRY_AFTER_SECS: &str = "5";

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, detail) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "Bad Request", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "Not Found", msg),
            ApiError::Unavailable(reason) => {
                // Expected under load, and one per refused request: `warn!`,
                // not `error!`, and never the underlying error text.
                warn!(reason, "request refused: over capacity");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service Unavailable",
                    reason.to_string(),
                )
            }
            ApiError::Internal(msg) => {
                // Log internal errors with full context but never expose
                // implementation details (DB connection strings, query
                // shapes, etc.) to the client.
                error!(error = %msg, "internal API error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                    "internal server error".to_string(),
                )
            }
        };

        let problem = ProblemDetails::generic(title, status.as_u16(), detail);

        // Hand-roll the response rather than using `axum::Json`, because
        // we need the RFC 9457 content type, not `application/json`.
        let body = serde_json::to_vec(&problem).expect("ProblemDetails always serialises");

        let mut response = (
            status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(PROBLEM_CONTENT_TYPE),
            )],
            body,
        )
            .into_response();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_static(RETRY_AFTER_SECS),
            );
        }
        response
    }
}

/// Convert a `RepositoryError` into an `ApiError`. Used pervasively in
/// handlers via the `?` operator on repository calls.
///
/// The mapping is intentionally coarse: every repository error becomes
/// `Internal`, except a timeout — the pool had no connection to give, or
/// Postgres cancelled a statement past its `statement_timeout` — which is the
/// API out of capacity, not broken: `Unavailable`. Application services are
/// responsible for translating `Ok(None)` into `ApiError::NotFound` — the
/// repository layer never propagates a not-found as an error.
impl From<yog_core::RepositoryError> for ApiError {
    fn from(err: yog_core::RepositoryError) -> Self {
        match err {
            yog_core::RepositoryError::Timeout(detail) => {
                // The detail can name a statement: logged, not sent.
                warn!(error = %detail, "repository timeout");
                ApiError::Unavailable(TIMEOUT_REASON)
            }
            other => ApiError::Internal(other.to_string()),
        }
    }
}

/// What a client is told when the database ran out of time or connections.
const TIMEOUT_REASON: &str = "the database is over capacity; retry shortly";

#[cfg(test)]
#[path = "tests/error_tests.rs"]
mod tests;
