//! RFC 9457 (Problem Details for HTTP APIs) response payload.
//!
//! Replaces the previous ad-hoc `{ "error": "msg" }` format with the
//! standardised Problem Details object. Conformance buys us:
//!
//!   - Native parsing in client libraries across languages (JS, Python,
//!     Go, Java, etc.)
//!   - First-class support in OpenAPI tooling
//!   - A dedicated content type — `application/problem+json` — that
//!     lets clients tell a payload of error apart from a payload of
//!     data without inspecting the status code
//!
//! The `type` field is set to `"about:blank"` at this stage: the RFC
//! explicitly allows this value when no per-error documentation URI
//! is offered. The `title` field then carries the human label and
//! the discrimination role. When we introduce specific error types
//! later (e.g. `invalid-cursor`), `type` will gain a real URI and
//! `title` will track it — no existing client breaks because every
//! field's contract is preserved.
//!
//! Reference: <https://www.rfc-editor.org/rfc/rfc9457>

use serde::Serialize;
use utoipa::ToSchema;

/// RFC 9457 content type. The handler sets this on the response,
/// not the DTO itself — [`axum::Json`] would force `application/json`.
pub(crate) const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// RFC 9457 Problem Details, the body of every error response. `type` is
/// `"about:blank"` until specific problem types exist, so clients branch on
/// `title`; `instance` is omitted, the client knows the path.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ProblemDetailsResponse {
    #[serde(rename = "type")]
    type_uri: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
}

impl ProblemDetailsResponse {
    /// Build a Problem Details payload with `type = "about:blank"`.
    /// `title` carries the discrimination role until specific type
    /// URIs are introduced.
    pub(crate) fn generic(title: &'static str, status: u16, detail: String) -> Self {
        Self {
            type_uri: "about:blank",
            title,
            status,
            detail,
        }
    }
}

#[cfg(test)]
#[path = "tests/problem_tests.rs"]
mod tests;
