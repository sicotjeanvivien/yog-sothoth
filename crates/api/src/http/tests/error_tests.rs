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

async fn detail_of(error: ApiError) -> (StatusCode, String) {
    let response = error.into_response();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    (status, json["detail"].as_str().unwrap().to_string())
}

/// Mutation: one message changed, or the two swapped so a refused stream speaks of a key.
#[tokio::test]
async fn each_credential_kind_says_what_it_requires() {
    use crate::application::CredentialKind;

    assert_eq!(
        detail_of(ApiError::Unauthorized(CredentialKind::ApiKey)).await,
        (
            StatusCode::UNAUTHORIZED,
            "a valid API key is required: send it as `Authorization: Bearer <key>`".to_string()
        )
    );
    assert_eq!(
        detail_of(ApiError::Unauthorized(CredentialKind::Ticket)).await,
        (
            StatusCode::UNAUTHORIZED,
            "a valid ticket is required: request one at `POST /api/auth/tickets` with your API key"
                .to_string()
        )
    );
}
