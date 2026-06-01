//! Unit tests for `GetTokenRequest::parse`.

use crate::http::dto::request::GetTokenRequest;
use crate::http::error::ApiError;

#[test]
fn accepts_valid_mint() {
    // SOL native mint.
    let mint = "So11111111111111111111111111111111111111112".to_string();
    let request = GetTokenRequest::parse(mint.clone()).expect("should parse");
    assert_eq!(request.mint.to_string(), mint);
}

#[test]
fn rejects_invalid_mint() {
    let err = GetTokenRequest::parse("not-base58".to_string()).unwrap_err();
    let msg = match err {
        ApiError::BadRequest(m) => m,
        other => panic!("expected BadRequest, got {other:?}"),
    };
    // The error message names "mint", not "pool address" — distinct
    // helper used.
    assert!(msg.contains("mint"));
}
