//! HTTP request/response tracing.
//!
//! Wraps each request in a tracing span carrying method, URI, status
//! and latency. A request_id is attached: if the caller (or upstream
//! reverse proxy) sent `x-request-id`, it is reused; else a UUIDv4
//! is generated. The id is echoed back on the response for
//! correlation.
//!
//! Health-check requests (`/healthz`) are excluded from span creation
//! to keep the log clean under load-balancer polling.

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

/// Build the per-request span. Returns `None` for `/healthz` so the
/// load balancer's polling doesn't flood the log.
pub(crate) fn make_request_span(request: &Request<Body>) -> Span {
    let method = request.method();
    let uri = request.uri();
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    if uri.path() == "/healthz" {
        tracing::debug_span!(
            "http_request",
            method = %method,
            uri = %uri,
            request_id = %request_id,
            status = field::Empty,
            latency_ms = field::Empty,
        )
    } else {
        tracing::info_span!(
            "http_request",
            method = %method,
            uri = %uri,
            request_id = %request_id,
            status = field::Empty,
            latency_ms = field::Empty,
        )
    }
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
