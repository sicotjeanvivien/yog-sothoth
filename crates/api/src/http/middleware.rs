//! HTTP middleware applied to every route: CORS and security headers.
pub(crate) mod authentication;
pub(crate) mod capacity;
pub(crate) mod live_authentication;
pub(super) mod problem_details;
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

/// CORS, limited to the origins of `API_CORS_ALLOWED_ORIGINS`; `x-request-id` is exposed so the
/// browser can report it. A caller sending no `Origin` is unaffected. `POST` and `Authorization`
/// are a visitor's own key, sent from the explorer, and the live token it exchanges it for.
pub(super) fn cors_layer(allowed_origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .expose_headers([HeaderName::from_static(REQUEST_ID_HEADER)])
}
