use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tracing::error;

use crate::http::dto::response::{PROBLEM_CONTENT_TYPE, ProblemDetails};

/// API-level errors surfaced by handlers.
///
/// Three variants cover everything we need at this layer:
/// - `BadRequest`: client supplied invalid input (bad cursor, out-of-range limit).
/// - `NotFound`: resource does not exist.
/// - `Internal`: unexpected failure — DB error, encoding bug, anything not the
///   client's fault. The detailed message is logged, never sent to the client.
///
/// Responses are serialised as RFC 9457 Problem Details with
/// `Content-Type: application/problem+json`. See
/// `crate::http::dto::response::problem` for the format rationale.
#[derive(Debug)]
pub(crate) enum ApiError {
    BadRequest(String),
    #[allow(dead_code)] // unused outside the pool/token handlers
    NotFound(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, detail) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "Bad Request", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "Not Found", msg),
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

        (
            status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(PROBLEM_CONTENT_TYPE),
            )],
            body,
        )
            .into_response()
    }
}

/// Convert a `RepositoryError` into an `ApiError`. Used pervasively in
/// handlers via the `?` operator on repository calls.
///
/// The mapping is intentionally coarse: every repository error becomes
/// `Internal`. Application services are responsible for translating
/// `Ok(None)` into `ApiError::NotFound` — the repository layer never
/// propagates a not-found as an error.
impl From<yog_core::RepositoryError> for ApiError {
    fn from(err: yog_core::RepositoryError) -> Self {
        ApiError::Internal(err.to_string())
    }
}
