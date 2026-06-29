//! HTTP middleware applied to every route.
//!
//! Currently configures CORS and a small set of security headers.
//! Future cross-cutting concerns (request tracing, rate limiting,
//! request ID propagation) will land here as additional layers.
pub(crate) mod tracing;

use axum::http::{HeaderName, HeaderValue, Method, header};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    set_header::SetResponseHeaderLayer,
};

use crate::http::middleware::tracing::REQUEST_ID_HEADER;

/// Build the middleware stack applied to the whole router.
///
/// The order of layers matters: layers added last are executed
/// outermost (i.e. closest to the network). For our cases the order
/// is irrelevant — neither security headers nor CORS depend on each
/// other — but documenting the convention now avoids confusion when
/// auth or rate limiting layers are added later.
pub(super) fn security_headers_layer() -> SetResponseHeaderLayer<HeaderValue> {
    // X-Content-Type-Options: nosniff
    // Tells the browser not to sniff the response body for a different
    // Content-Type than the one we sent. Cheap defense against MIME
    // confusion attacks.
    SetResponseHeaderLayer::if_not_present(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    )
}

pub(super) fn frame_options_layer() -> SetResponseHeaderLayer<HeaderValue> {
    // X-Frame-Options: DENY
    // Prevents the API from being embedded in an iframe. The dashboard
    // is a separate origin; there's no legitimate reason to frame this
    // API, so we reject all framing categorically.
    SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    )
}

/// CORS configuration.
///
/// Restricted to the explicit set of browser origins configured via
/// `API_CORS_ALLOWED_ORIGINS` (parsed at boot in `bootstrap::config`).
/// The API is read-only, so only `GET` is allowed; `Content-Type` is
/// the sole request header a browser sets on these calls. The
/// `x-request-id` response header is exposed so the browser client can
/// surface the correlation id when reporting a server error.
///
/// Non-browser callers (curl, the SSR layer via `API_INTERNAL_URL`,
/// monitoring) don't send an `Origin` header and are unaffected — CORS
/// only ever *grants* cross-origin browser access, it never gates
/// server-to-server traffic.
pub(super) fn cors_layer(allowed_origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET])
        .allow_headers([header::CONTENT_TYPE])
        .expose_headers([HeaderName::from_static(REQUEST_ID_HEADER)])
}
