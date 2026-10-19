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
    assert_eq!(json["detail"], "Method Not Allowed");
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

/// Mutation: an oversized error body buffered and echoed instead of dropped.
#[tokio::test]
async fn an_oversized_error_body_is_not_echoed() {
    let text = "x".repeat(super::MAX_ERROR_BODY + 1);
    let answer =
        answer_in_problem_details(response(StatusCode::BAD_REQUEST, Some("text/plain"), &text))
            .await;
    assert_eq!(problem(answer).await["detail"], "Bad Request");
}

/// Mutation: the layer rewrites a JSON error body — the readiness report of `/readyz`, or
/// a Problem Details whose `Retry-After` would be lost.
#[tokio::test]
async fn a_json_error_body_passes_as_is() {
    for content_type in ["application/json", "application/problem+json"] {
        let mut answer = response(
            StatusCode::SERVICE_UNAVAILABLE,
            Some(content_type),
            r#"{"status":"unavailable"}"#,
        );
        answer
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
        let answer = answer_in_problem_details(answer).await;
        assert_eq!(answer.headers()[header::CONTENT_TYPE], content_type);
        assert_eq!(answer.headers()[header::RETRY_AFTER], "5");
        assert_eq!(body(answer).await, r#"{"status":"unavailable"}"#);
    }
}

/// Mutation: the layer touches a success.
#[tokio::test]
async fn a_success_passes_as_is() {
    let answer =
        answer_in_problem_details(response(StatusCode::OK, Some("text/plain"), "ok")).await;
    assert_eq!(answer.headers()[header::CONTENT_TYPE], "text/plain");
    assert_eq!(body(answer).await, "ok");
}

/// Mutation: the layer mounted inside the routes only, leaving the fallback's empty `404`.
#[tokio::test]
async fn an_unknown_route_is_answered_in_problem_details() {
    let (status, content_type, _) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
}

#[tokio::test]
async fn a_wrong_method_is_answered_in_problem_details() {
    let (status, headers, _) = call(Method::POST, "/api/signals").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert!(headers.contains_key(header::ALLOW));
}

/// Mutation: the layer rewrites `/readyz`, whose `503` carries its own report.
#[tokio::test]
async fn readyz_keeps_its_own_report() {
    let (status, content_type, _) = get("/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(content_type, "application/json");
}
