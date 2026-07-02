//! Unit tests for `ListSignalsRequest::parse`.

use chrono::{TimeZone, Utc};
use yog_core::domain::{Severity, SignalCursor};
use yog_core::{Cursor, PageDirection};

use crate::http::cursor::encode_cursor;
use crate::http::dto::request::ListSignalsRequest;
use crate::http::error::ApiError;
use crate::http::query::{
    PageDirectionParam, PagePositionParam, SeverityParam, SignalsQuery, default_limit,
};

fn valid_query() -> SignalsQuery {
    SignalsQuery {
        cursor: None,
        dir: PageDirectionParam::Next,
        position: None,
        severity: None,
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
    let mut q = valid_query();
    q.cursor = Some("any".to_string());
    q.position = Some(PagePositionParam::First);
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_malformed_cursor() {
    let mut q = valid_query();
    q.cursor = Some("not-base64!!".to_string());
    let err = ListSignalsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}
