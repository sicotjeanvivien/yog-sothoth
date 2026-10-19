//! Unit tests for `ListSignalsRequest::parse` and its validators.

use chrono::{TimeZone, Utc};
use yog_core::domain::{Severity, SignalCursor};
use yog_core::{Cursor, PageDirection};

use super::signals_query::default_limit;
use super::{
    ListSignalsRequest, MAX_LIMIT, PageDirectionParam, PagePositionParam, SeverityParam,
    SignalsQuery, validate_cursor_position_exclusive, validate_limit,
};
use crate::http::cursor::encode_cursor;
use crate::http::error::ApiError;

fn valid_query() -> SignalsQuery {
    SignalsQuery {
        cursor: None,
        dir: PageDirectionParam::Next,
        position: None,
        severity: None,
        pool: None,
        limit: default_limit(),
    }
}

#[test]
fn parses_default_query() {
    let request = ListSignalsRequest::parse(valid_query()).unwrap();
    let params = request.into_params();
    assert!(params.severity.is_none());
    assert!(params.cursor.is_none());
    assert_eq!(params.direction, PageDirection::Next);
    assert_eq!(params.limit, 50);
}

#[test]
fn maps_the_severity_filter() {
    let mut q = valid_query();
    q.severity = Some(SeverityParam::Critical);
    let params = ListSignalsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.severity, Some(Severity::Critical));
}

#[test]
fn maps_the_pool_filter() {
    let mut q = valid_query();
    let address = solana_pubkey::Pubkey::new_from_array([7; 32]);
    q.pool = Some(address.to_string());
    let params = ListSignalsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.pool, Some(address));
}

#[test]
fn rejects_an_invalid_pool_address() {
    let mut q = valid_query();
    q.pool = Some("not-a-pubkey".to_string());
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(m) if m == "invalid pool address: not-a-pubkey"));
}

/// Mutation: `?pool=` read as an absent filter.
#[test]
fn rejects_an_empty_pool() {
    let mut q = valid_query();
    q.pool = Some(String::new());
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(m) if m == "`pool` must not be empty"));
}

/// Mutation: `?cursor=` read as an absent cursor.
#[test]
fn rejects_an_empty_cursor() {
    let mut q = valid_query();
    q.cursor = Some(String::new());
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(m) if m == "`cursor` must not be empty"));
}

/// Mutation: the exclusivity checked before the cursor is decoded.
#[test]
fn an_empty_cursor_beside_a_position_is_refused_for_the_cursor() {
    let mut q = valid_query();
    q.cursor = Some(String::new());
    q.position = Some(PagePositionParam::Last);
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(m) if m == "`cursor` must not be empty"));
}

#[test]
fn roundtrips_an_encoded_cursor() {
    let cursor = SignalCursor {
        triggered_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        id: 42,
    };
    let mut q = valid_query();
    q.cursor = Some(encode_cursor(&Cursor::Signal(cursor.clone())).unwrap());

    let params = ListSignalsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.cursor, Some(cursor));
}

#[test]
fn rejects_invalid_limit() {
    let mut q = valid_query();
    q.limit = 0;
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_cursor_position_combination() {
    let cursor = SignalCursor {
        triggered_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        id: 42,
    };
    let mut q = valid_query();
    q.cursor = Some(encode_cursor(&Cursor::Signal(cursor)).unwrap());
    q.position = Some(PagePositionParam::First);
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(
        matches!(err, ApiError::BadRequest(m) if m == "`position` cannot be combined with `cursor`")
    );
}

#[test]
fn rejects_malformed_cursor() {
    let mut q = valid_query();
    q.cursor = Some("not-base64!!".to_string());
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn validate_limit_bounds() {
    assert!(validate_limit(0).is_err());
    assert!(validate_limit(1).is_ok());
    assert!(validate_limit(MAX_LIMIT).is_ok());
    assert!(validate_limit(MAX_LIMIT + 1).is_err());
}

#[test]
fn validate_pagination_rejects_cursor_with_position() {
    assert!(validate_cursor_position_exclusive(true, true).is_err());
}

#[test]
fn validate_pagination_allows_cursor_alone() {
    assert!(validate_cursor_position_exclusive(true, false).is_ok());
}
