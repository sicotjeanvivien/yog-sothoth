//! A key required, where `authenticate` lets an anonymous request through: the route reads the
//! client the key names. ⚠️ Mounted under `authenticate`, which attaches that client.

use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use yog_core::domain::ApiClient;

use crate::{
    application::{CredentialKind, Unauthorized, metrics::ApiMetrics},
    http::error::ApiError,
};

/// Refuse a request no key has named a client for.
pub(crate) async fn require_api_key(request: Request, next: Next) -> Response {
    if request.extensions().get::<ApiClient>().is_none() {
        ApiMetrics::record_unauthorized(CredentialKind::ApiKey, Unauthorized::Missing);
        return ApiError::Unauthorized(CredentialKind::ApiKey).into_response();
    }
    next.run(request).await
}

#[cfg(test)]
#[path = "require_api_key_tests.rs"]
mod tests;
