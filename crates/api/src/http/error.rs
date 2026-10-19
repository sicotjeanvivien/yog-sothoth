use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tracing::{error, warn};

use crate::http::dto::response::{PROBLEM_CONTENT_TYPE, ProblemDetailsResponse};

/// API-level errors, served as RFC 9457 Problem Details. `Unavailable` is capacity or time
/// running out — `503` with `Retry-After`, not `500`; `Internal` is logged, never detailed.
#[derive(Debug)]
pub(crate) enum ApiError {
    BadRequest(String),
    #[allow(dead_code)] // unused outside the pool/token handlers
    NotFound(String),
    Unavailable(&'static str),
    Internal(String),
    /// What axum answered outside `ApiError` — an extractor's refusal, an unknown route — its
    /// status kept. A `5xx`'s text is logged, never sent.
    Framework(StatusCode, String),
}

/// The only `detail` a `5xx` ever carries.
const INTERNAL_DETAIL: &str = "internal server error";

/// What `Retry-After` tells a refused client, in seconds: about the time a slot takes to free.
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
                // Logged in full, never sent.
                error!(error = %msg, "internal API error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                    INTERNAL_DETAIL.to_string(),
                )
            }
            ApiError::Framework(status, text) => {
                let title = status.canonical_reason().unwrap_or("Error");
                if status.is_server_error() {
                    error!(%status, error = %text, "framework error");
                    (status, title, INTERNAL_DETAIL.to_string())
                } else if text.is_empty() {
                    (status, title, title.to_string())
                } else {
                    (status, title, text)
                }
            }
        };

        let problem = ProblemDetailsResponse::generic(title, status.as_u16(), detail);

        // Not `axum::Json`: RFC 9457 needs its own content type.
        let body = serde_json::to_vec(&problem).expect("ProblemDetailsResponse always serialises");

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

/// Every repository error is `Internal` but a timeout — no connection, or a statement past its
/// `statement_timeout` — which is `Unavailable`. A not-found is the services' `Ok(None)`.
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
