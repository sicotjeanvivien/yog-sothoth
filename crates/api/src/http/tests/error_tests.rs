use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use yog_core::RepositoryError;

use super::ApiError;

/// A timeout — no connection in time, or a statement cancelled by
/// `statement_timeout` — is the API over capacity: 503 with `Retry-After`,
/// in Problem Details, and the underlying detail stays in the log.
#[tokio::test]
async fn a_repository_timeout_is_a_503_with_retry_after() {
    let response = ApiError::from(RepositoryError::Timeout(
        "canceling statement due to statement timeout".into(),
    ))
    .into_response();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(response.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        !text.contains("statement timeout"),
        "the repository's own text must not reach the client: {text}"
    );
}

/// Only a timeout: a backend failure is still a 500, and carries no
/// `Retry-After` — retrying it would change nothing.
#[test]
fn a_backend_failure_stays_a_500() {
    let response = ApiError::from(RepositoryError::Backend("boom".into())).into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!response.headers().contains_key(header::RETRY_AFTER));
}
