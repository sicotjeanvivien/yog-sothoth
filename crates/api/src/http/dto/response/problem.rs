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

/// RFC 9457 content type. The handler sets this on the response,
/// not the DTO itself — `axum::Json` would force `application/json`.
pub(crate) const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// Problem Details for HTTP APIs — the wire shape returned for every
/// error response.
///
/// Field semantics, all per RFC 9457 §3:
///
///   - `type` — URI reference identifying the problem type. Opaque
///     to clients except for equality comparison. `"about:blank"` is
///     the reserved value meaning "no specific type, see `title`".
///   - `title` — short, human-readable summary. Should not change
///     across occurrences of the same problem type. Clients can
///     branch on `(type, title)` together.
///   - `status` — the HTTP status code. Redundant with the response
///     line, but useful for clients that log only the body.
///   - `detail` — human-readable per-occurrence message. May vary
///     across occurrences of the same type. This is where the
///     contextual information lives ("invalid pool address: foo").
///
/// `instance` is intentionally omitted: it would carry the request
/// path, which the client already knows. It can be added later
/// without breaking the contract (it's an optional field).
#[derive(Debug, Serialize)]
pub(crate) struct ProblemDetails {
    #[serde(rename = "type")]
    pub(crate) type_uri: &'static str,
    pub(crate) title: &'static str,
    pub(crate) status: u16,
    pub(crate) detail: String,
}

impl ProblemDetails {
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
