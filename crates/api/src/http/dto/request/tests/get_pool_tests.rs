//! Unit tests for `GetPoolRequest::parse`.

use crate::http::dto::request::GetPoolRequest;
use crate::http::error::ApiError;

#[test]
fn accepts_valid_base58_address() {
    // A well-known mainnet address from the watched_pools seed.
    let addr = "AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv".to_string();
    let request = GetPoolRequest::parse(addr.clone()).expect("should parse");
    assert_eq!(request.pool_address.to_string(), addr);
}

#[test]
fn rejects_invalid_base58() {
    let err = GetPoolRequest::parse("not-a-pubkey".to_string()).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_empty_string() {
    let err = GetPoolRequest::parse(String::new()).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_too_short_address() {
    let err = GetPoolRequest::parse("abc".to_string()).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}
