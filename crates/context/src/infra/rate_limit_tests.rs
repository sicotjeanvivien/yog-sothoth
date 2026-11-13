use serde_json::json;

use super::*;
use crate::testing::ScriptedProvider;

/// One POST to a server that answers `status`.
async fn response_with(status: u16) -> reqwest::Response {
    let provider = ScriptedProvider::answering(vec![(status, json!({}))]).await;
    reqwest::Client::new()
        .post(provider.url.expose())
        .json(&json!({}))
        .send()
        .await
        .expect("the server answers")
}

fn headers_with_retry_after(value: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::RETRY_AFTER,
        value.parse().expect("valid header value"),
    );
    headers
}

/// Mutation: the status compared to another code, or the check removed.
#[tokio::test]
async fn a_429_is_rate_limited() {
    let refused = refused(response_with(429).await);

    assert!(
        matches!(refused, Err(SourceError::RateLimited { retry_after: None })),
        "got {refused:?}"
    );
}

/// Mutation: every response refused.
#[tokio::test]
async fn any_other_status_passes_through() {
    let passed = refused(response_with(500).await).expect("not a 429");

    assert_eq!(passed.status(), reqwest::StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn retry_after_parses_delta_seconds() {
    let headers = headers_with_retry_after("2");
    assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(2)));
}

#[test]
fn retry_after_absent_yields_none() {
    let headers = reqwest::header::HeaderMap::new();
    assert_eq!(parse_retry_after(&headers), None);
}

#[test]
fn retry_after_http_date_form_yields_none() {
    let headers = headers_with_retry_after("Wed, 21 Oct 2026 07:28:00 GMT");
    assert_eq!(parse_retry_after(&headers), None);
}
