//! HTTP middleware applied to every route: CORS and security headers.
pub(crate) mod authentication;
pub(crate) mod capacity;
pub(super) mod problem_details;
pub(crate) mod require_api_key;
pub(crate) mod ticket_authentication;
pub(crate) mod tracing;

use axum::http::{HeaderName, HeaderValue, Method, header};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    set_header::SetResponseHeaderLayer,
};

use crate::http::middleware::tracing::REQUEST_ID_HEADER;

/// The `X-Content-Type-Options: nosniff` header.
pub(super) fn security_headers_layer() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    )
}

pub(super) fn frame_options_layer() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    )
}

/// CORS, limited to the origins of `API_CORS_ALLOWED_ORIGINS`, to `GET` and to the `POST` of a
/// ticket; `Authorization` carries a key or a ticket. A caller sending no `Origin` is unaffected.
pub(super) fn cors_layer(allowed_origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        // ⚠️ A browser reads no other response header: `x-request-id` to report, `Retry-After` to wait.
        .expose_headers([
            HeaderName::from_static(REQUEST_ID_HEADER),
            header::RETRY_AFTER,
        ])
}

#[cfg(test)]
#[path = "middleware/cors_tests.rs"]
mod cors_tests;
