//! Every error of an `/api` route leaves as RFC 9457 Problem Details: what axum answers on
//! its own — an extractor's refusal, a wrong method — is rewritten here.

use axum::{
    body::to_bytes,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use tracing::error;

use crate::http::dto::response::PROBLEM_CONTENT_TYPE;
use crate::http::error::ApiError;

/// An error body past this is not echoed: its text is dropped.
const MAX_ERROR_BODY: usize = 4 * 1024;

/// An error response that is not Problem Details, rewritten as one. Its own headers win
/// over the rewrite's (a `Retry-After`), except the two that describe the old body.
pub(crate) async fn answer_in_problem_details(response: Response) -> Response {
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error())
        || is_problem_details(response.headers())
    {
        return response;
    }
    let (parts, body) = response.into_parts();
    let text = match to_bytes(body, MAX_ERROR_BODY).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    };
    if status.is_server_error() {
        error!(%status, error = %text, "framework error");
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

fn is_problem_details(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case(PROBLEM_CONTENT_TYPE))
}

#[cfg(test)]
#[path = "problem_details_tests.rs"]
mod tests;
