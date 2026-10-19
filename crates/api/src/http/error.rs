use axum::{
    extract::rejection::{PathRejection, QueryRejection},
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
}

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
                    "internal server error".to_string(),
                )
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

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        Self::rejected(rejection.status(), rejection.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        Self::rejected(rejection.status(), rejection.body_text())
    }
}

impl ApiError {
    /// An extractor's refusal, its text as the `detail`. ⚠️ Not all `400`: a path rejection
    /// can be a routing bug (`500`), which stays internal.
    fn rejected(status: StatusCode, text: String) -> Self {
        if status.is_client_error() {
            ApiError::BadRequest(text)
        } else {
            ApiError::Internal(text)
        }
    }
}

#[cfg(test)]
#[path = "tests/error_tests.rs"]
mod tests;
