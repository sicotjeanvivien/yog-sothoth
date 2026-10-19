//! Every error leaves as RFC 9457 Problem Details: what axum answers on its own — an
//! extractor's refusal, an unknown route, a wrong method — is rewritten here.

use axum::{
    body::to_bytes,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use crate::http::error::ApiError;

/// An error body past this is not read: its text is dropped, never buffered.
const MAX_ERROR_BODY: usize = 4 * 1024;

/// An error response that is not JSON, rewritten as Problem Details with its status and
/// headers. ⚠️ A JSON error body passes as is: it is the route's declared answer (`ApiError`,
/// `/readyz`), so it must be one.
pub(crate) async fn answer_in_problem_details(response: Response) -> Response {
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error()) || is_json(response.headers()) {
        return response;
    }
    let (parts, body) = response.into_parts();
    let text = match to_bytes(body, MAX_ERROR_BODY).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    };
    let mut answer = ApiError::Framework(status, text).into_response();
    for name in parts.headers.keys() {
        if name != header::CONTENT_TYPE
            && name != header::CONTENT_LENGTH
            && !answer.headers().contains_key(name)
        {
            for value in parts.headers.get_all(name) {
                answer.headers_mut().append(name, value.clone());
            }
        }
    }
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
