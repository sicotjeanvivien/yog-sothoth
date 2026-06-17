//! Unit tests for `GetPoolHistoryRequest::parse`.

use crate::http::dto::request::GetPoolHistoryRequest;
use crate::http::error::ApiError;
use crate::http::query::HistoryQuery;

const VALID: &str = "AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv";

#[test]
fn accepts_valid_address_and_days() {
    let req = GetPoolHistoryRequest::parse(VALID.to_string(), HistoryQuery { days: 30 })
        .expect("should parse");
    assert_eq!(req.pool_address.to_string(), VALID);
    assert_eq!(req.days, 30);
}

#[test]
fn rejects_invalid_address() {
    let err =
        GetPoolHistoryRequest::parse("nope".to_string(), HistoryQuery { days: 7 }).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_zero_days() {
    let err =
        GetPoolHistoryRequest::parse(VALID.to_string(), HistoryQuery { days: 0 }).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_out_of_range_days() {
    let err =
        GetPoolHistoryRequest::parse(VALID.to_string(), HistoryQuery { days: 9999 }).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}
