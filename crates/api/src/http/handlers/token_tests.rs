use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use crate::http::build_router;
use crate::testing::app_state;

async fn get(uri: &str) -> (StatusCode, String) {
    let response = build_router(app_state(), vec![])
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let content_type = response.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .to_string();
    (response.status(), content_type)
}

/// Mutation: the handler extracts a bare `Path`, whose refusal of a non-UTF-8 segment is
/// plain text.
#[tokio::test]
async fn a_path_the_api_cannot_parse_is_refused_in_problem_details() {
    let (status, content_type) = get("/api/tokens/%FF").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(content_type, "application/problem+json");
}

/// The control: a well-formed mint reaches the handler, which finds no such token.
#[tokio::test]
async fn a_well_formed_mint_reaches_the_handler() {
    let (status, content_type) =
        get("/api/tokens/So11111111111111111111111111111111111111112").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
}
