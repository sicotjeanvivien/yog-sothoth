use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::error;

/// API-level errors surfaced by handlers.
///
/// Three variants cover everything we need at this layer:
/// - `BadRequest`: client supplied invalid input (bad cursor, out-of-range limit).
/// - `NotFound`: resource does not exist (used by future endpoints with path params).
/// - `Internal`: unexpected failure — DB error, encoding bug, anything not the
///   client's fault. The detailed message is logged, never sent to the client.
#[derive(Debug)]
pub(crate) enum ApiError {
    BadRequest(String),
    #[allow(dead_code)] // unused in commit 2, used by future endpoints
    NotFound(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Internal(msg) => {
                // Log internal errors with full context but never expose
                // implementation details (DB connection strings, query
                // shapes, etc.) to the client.
                error!(error = %msg, "internal API error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// Convert a `RepositoryError` into an `ApiError`. Used pervasively in
/// handlers via the `?` operator on repository calls.
///
/// The mapping is intentionally coarse: most repository errors are
/// internal (connection issues, schema corruption, query failures).
/// `NotFound` is the only one that has a natural client-facing meaning,
/// but we don't surface it from `find_paginated` (an empty page is fine,
/// not a 404).
impl From<yog_core::RepositoryError> for ApiError {
    fn from(err: yog_core::RepositoryError) -> Self {
        ApiError::Internal(err.to_string())
    }
}
