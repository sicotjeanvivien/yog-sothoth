//! Every error leaves as RFC 9457 Problem Details: what axum answers on its own — an
//! extractor's refusal, an unknown route, a wrong method — is rewritten here.

use axum::{
    body::to_bytes,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use tracing::error;

use crate::http::error::ApiError;
use crate::http::middleware::tracing::REQUEST_ID_HEADER;

/// An error body past this is not read: its text is dropped, never buffered.
const MAX_ERROR_BODY: usize = 4 * 1024;

/// An error response that is not JSON, rewritten as Problem Details. Its own headers win
/// over the rewrite's (a `Retry-After`), except the two that describe the old body.
pub(crate) async fn answer_in_problem_details(response: Response) -> Response {
    let status = response.status();
    // ⚠️ A JSON error body is the route's declared answer (`ApiError`, `/readyz`): it passes
    // as is, so it must be Problem Details or the route's documented report.
    if !(status.is_client_error() || status.is_server_error()) || is_json(response.headers()) {
        return response;
    }
    let (parts, body) = response.into_parts();
    let text = match to_bytes(body, MAX_ERROR_BODY).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    };
    if status.is_server_error() {
        // Outside the request's span: the id is read off the response to correlate the log.
        let request_id = parts
            .headers
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok());
        error!(%status, request_id, error = %text, "framework error");
    }
    let mut headers = parts.headers;
    headers.remove(header::CONTENT_TYPE);
    headers.remove(header::CONTENT_LENGTH);
    let mut answer = ApiError::Framework(status, text).into_response();
    for (name, value) in answer.headers() {
        headers.entry(name).or_insert_with(|| value.clone());
    }
    *answer.headers_mut() = headers;
    answer
}

/// `application/json`, or any `+json` type such as `application/problem+json`.
fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|mime| mime.trim().to_ascii_lowercase())
        .is_some_and(|mime| mime == "application/json" || mime.ends_with("+json"))
}

#[cfg(test)]
#[path = "problem_details_tests.rs"]
mod tests;
