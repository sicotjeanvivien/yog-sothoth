//! Unit tests for `GetPoolLatestStateRequest::parse`.

use crate::http::dto::request::GetPoolLatestStateRequest;
use crate::http::error::ApiError;

#[test]
fn accepts_valid_address_and_preserves_raw() {
    let raw = "AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv".to_string();
    let request = GetPoolLatestStateRequest::parse(raw.clone()).expect("should parse");

    // The raw string is preserved verbatim — the projection table is
    // keyed by TEXT in persistence.
    assert_eq!(request.raw_address, raw);
    assert_eq!(request.pool_address.to_string(), raw);
}

#[test]
fn rejects_invalid_address() {
    let err = GetPoolLatestStateRequest::parse("garbage".to_string()).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}
