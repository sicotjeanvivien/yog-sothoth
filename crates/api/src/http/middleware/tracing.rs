//! HTTP request tracing: a span per request with method, URI, status, latency and a request
//! id — the caller's `x-request-id` or a fresh UUIDv4, echoed back. ⚠️ The probes are kept off
//! this layer in [`http::build_router`]: `debug_span!` would not lower the `info!` events inside.
//!
//! [`http::build_router`]: crate::http::build_router

use std::time::Duration;

use axum::{body::Body, extract::Request, http::Response};
use tower_http::request_id::{MakeRequestId, RequestId};
use tracing::{Span, field};
use uuid::Uuid;

pub(crate) const REQUEST_ID_HEADER: &str = "x-request-id";

/// Generates a UUIDv4 when no `x-request-id` was provided by the caller.
#[derive(Clone, Default)]
pub(crate) struct GenerateRequestId;

impl MakeRequestId for GenerateRequestId {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let id = Uuid::new_v4().to_string();
        axum::http::HeaderValue::from_str(&id)
            .ok()
            .map(RequestId::new)
    }
}

/// The per-request span, with `status` and `latency_ms` recorded later.
pub(crate) fn make_request_span(request: &Request<Body>) -> Span {
    let method = request.method();
    let uri = request.uri();
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    tracing::info_span!(
        "http_request",
        method = %method,
        uri = %uri,
        request_id = %request_id,
        status = field::Empty,
        latency_ms = field::Empty,
    )
}

pub(crate) fn on_request(_request: &Request<Body>, _span: &Span) {
    tracing::debug!("request started");
}

pub(crate) fn on_response(response: &Response<Body>, latency: Duration, span: &Span) {
    span.record("status", response.status().as_u16());
    span.record("latency_ms", latency.as_millis() as u64);
    tracing::info!("request completed");
}

pub(crate) fn on_failure(
    error: tower_http::classify::ServerErrorsFailureClass,
    latency: Duration,
    _span: &Span,
) {
    tracing::warn!(
        error = %format!("{error}"),
        latency_ms = latency.as_millis() as u64,
        "request failed",
    );
}
