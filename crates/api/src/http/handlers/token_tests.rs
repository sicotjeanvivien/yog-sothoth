use axum::http::StatusCode;

use crate::testing::get;

/// Mutation: the router without its Problem Details layer, axum's refusal of a non-UTF-8
/// segment left in plain text.
#[tokio::test]
async fn a_path_the_api_cannot_parse_is_refused_in_problem_details() {
    let (status, content_type, detail) = get("/api/tokens/%FF").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(content_type, "application/problem+json");
    assert!(detail.contains("`mint`"), "{detail}");
}

/// The control: a well-formed mint reaches the handler, which finds no such token.
#[tokio::test]
async fn a_well_formed_mint_reaches_the_handler() {
    let (status, content_type, _) =
        get("/api/tokens/So11111111111111111111111111111111111111112").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
}
