//! RFC 9457 Problem Details, the body of every error response
//! (<https://www.rfc-editor.org/rfc/rfc9457>).

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
    /// A Problem Details payload with `type = "about:blank"`.
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
