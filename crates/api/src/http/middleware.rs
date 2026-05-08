//! HTTP middleware applied to every route.
//!
//! Currently configures CORS and a small set of security headers.
//! Future cross-cutting concerns (request tracing, rate limiting,
//! request ID propagation) will land here as additional layers.

use axum::http::{HeaderName, HeaderValue, header};
use tower_http::{cors::CorsLayer, set_header::SetResponseHeaderLayer};

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
/// `permissive()` allows any origin, any method, any header. Suitable
/// for development and for the current state where no production
/// frontend exists yet. Tighten this once the dashboard is deployed —
/// typically: `CorsLayer::new().allow_origin("https://yog-sothoth.fr".parse()?)`.
pub(super) fn cors_layer() -> CorsLayer {
    // TODO(v0.1 step 4): restrict to the dashboard origin once Next.js
    // is deployed. Until then, permissive CORS unblocks local dev with
    // any tool (curl, Postman, browser).
    CorsLayer::permissive()
}
