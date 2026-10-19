use axum::body::{Body, to_bytes};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::Response;

use super::answer_in_problem_details;
use crate::testing::{call, get};

fn response(status: StatusCode, content_type: Option<&str>, body: &str) -> Response {
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() = status;
    if let Some(content_type) = content_type {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type).unwrap(),
        );
    }
    response
}

async fn body(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn problem(response: Response) -> serde_json::Value {
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    serde_json::from_str(&body(response).await).unwrap()
}

/// Mutation: the layer drops the text of a refusal, which names the field at fault.
#[tokio::test]
async fn a_plain_text_refusal_becomes_problem_details_with_its_text() {
    let text = "Failed to deserialize query string: limit: invalid digit found in string";
    let answer = answer_in_problem_details(response(
        StatusCode::BAD_REQUEST,
        Some("text/plain; charset=utf-8"),
        text,
    ))
    .await;
    assert_eq!(answer.status(), StatusCode::BAD_REQUEST);
    let json = problem(answer).await;
    assert_eq!(json["status"], 400);
    assert_eq!(json["title"], "Bad Request");
    assert_eq!(json["detail"], text);
}

/// Mutation: the original headers lost — a `405` without `Allow` names no method to use.
#[tokio::test]
async fn an_empty_refusal_keeps_its_status_and_headers() {
    let mut refused = response(StatusCode::METHOD_NOT_ALLOWED, None, "");
    refused
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("GET,HEAD"));
    let answer = answer_in_problem_details(refused).await;
    assert_eq!(answer.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(answer.headers()[header::ALLOW], "GET,HEAD");
    let json = problem(answer).await;
    assert_eq!(json["title"], "Method Not Allowed");
    assert_eq!(json["detail"], "method not allowed");
}

/// Mutation: a `5xx` sent with its own text, which can name the server's internals.
#[tokio::test]
async fn a_server_error_is_sent_without_its_text() {
    let answer = answer_in_problem_details(response(
        StatusCode::INTERNAL_SERVER_ERROR,
        Some("text/plain"),
        "No paths parameters found for matched route",
    ))
    .await;
    assert_eq!(answer.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let json = problem(answer).await;
    assert_eq!(json["detail"], "internal server error");
}

/// Mutation: the rewrite's own `Retry-After` wins over the one the refusal carried.
#[tokio::test]
async fn a_plain_text_503_keeps_its_own_retry_after() {
    let mut refused = response(StatusCode::SERVICE_UNAVAILABLE, Some("text/plain"), "shed");
    refused
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("30"));
    let answer = answer_in_problem_details(refused).await;
    assert_eq!(answer.headers()[header::RETRY_AFTER], "30");
    assert_eq!(problem(answer).await["detail"], "service unavailable");
}

/// Mutation: an oversized error body buffered and echoed instead of dropped.
#[tokio::test]
async fn an_oversized_error_body_is_not_echoed() {
    let text = "x".repeat(super::MAX_ERROR_BODY + 1);
    let answer =
        answer_in_problem_details(response(StatusCode::BAD_REQUEST, Some("text/plain"), &text))
            .await;
    assert_eq!(problem(answer).await["detail"], "bad request");
}

/// Mutation: the layer rewrites one of our own Problem Details, and its `detail` with it.
#[tokio::test]
async fn a_problem_details_body_passes_as_is() {
    let text = r#"{"title":"Service Unavailable","status":503,"detail":"at capacity"}"#;
    let answer = answer_in_problem_details(response(
        StatusCode::SERVICE_UNAVAILABLE,
        Some("application/problem+json"),
        text,
    ))
    .await;
    assert_eq!(body(answer).await, text);
}

/// Mutation: any JSON error body let through — a handler's `(400, Json(dto))` would leave
/// outside the format the contract declares.
#[tokio::test]
async fn a_json_error_body_that_is_not_problem_details_is_rewritten() {
    let answer = answer_in_problem_details(response(
        StatusCode::BAD_REQUEST,
        Some("application/json"),
        r#"{"error":"nope"}"#,
    ))
    .await;
    assert_eq!(problem(answer).await["detail"], r#"{"error":"nope"}"#);
}

/// Mutation: the layer touches a success.
#[tokio::test]
async fn a_success_passes_as_is() {
    let answer =
        answer_in_problem_details(response(StatusCode::OK, Some("text/plain"), "ok")).await;
    assert_eq!(answer.headers()[header::CONTENT_TYPE], "text/plain");
    assert_eq!(body(answer).await, "ok");
}

/// Mutation: the router without its own fallback, the `404` left to axum's default.
#[tokio::test]
async fn an_unknown_route_is_answered_in_problem_details() {
    let (status, content_type, detail) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
    assert_eq!(detail, "no such route");
}

#[tokio::test]
async fn a_wrong_method_is_answered_in_problem_details() {
    let (status, headers, _) = call(Method::POST, "/api/signals").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert!(headers.contains_key(header::ALLOW));
}

/// Mutation: the probes mounted under the layer, which rewrites the report of `/readyz`.
#[tokio::test]
async fn readyz_keeps_its_own_report() {
    let (status, content_type, _) = get("/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(content_type, "application/json");
}
