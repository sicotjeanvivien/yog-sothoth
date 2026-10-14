//! Unit tests for `ListPoolSwapsRequest::parse`.

use crate::http::dto::request::ListPoolSwapsRequest;
use crate::http::dto::request::test_common::valid_event_page_query;
use crate::http::error::ApiError;
use crate::http::query::EventPageQuery;
use crate::http::query::PageDirectionParam;
use yog_core::PageDirection;

const VALID_ADDR: &str = "AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv";

#[test]
fn parses_valid_address_and_query() {
    let request =
        ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), valid_event_page_query()).unwrap();
    let params = request.into_params();
    assert_eq!(params.pool_address.to_string(), VALID_ADDR);
    assert!(params.cursor.is_none());
    assert_eq!(params.limit, 50);
}

#[test]
fn rejects_invalid_pool_address() {
    let err =
        ListPoolSwapsRequest::parse("garbage".to_string(), valid_event_page_query()).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_invalid_limit() {
    let mut q = valid_event_page_query();
    q.limit = 0;
    let err = ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_cursor_position_combination() {
    let mut q = valid_event_page_query();
    q.cursor = Some("any".to_string());
    q.position = Some(crate::http::query::PagePositionParam::First);
    let err = ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_malformed_cursor() {
    let mut q = valid_event_page_query();
    q.cursor = Some("not-base64!!".to_string());
    let err = ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn empty_cursor_is_treated_as_none() {
    let mut q = valid_event_page_query();
    q.cursor = Some(String::new());
    let params = ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), q)
        .unwrap()
        .into_params();
    assert!(params.cursor.is_none());
}

#[test]
fn direction_is_threaded_through() {
    let mut q = valid_event_page_query();
    q.dir = PageDirectionParam::Prev;
    let params = ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), q)
        .unwrap()
        .into_params();
    assert_eq!(params.direction, PageDirection::Prev);
}

#[test]
fn pool_list_params_are_ignored_not_refused() {
    // Red if the event feed starts refusing the pool list's params (`deny_unknown_fields`).
    let uri: axum::http::Uri = "/x?q=SOL&sort=bogus&fee_bps=25".parse().unwrap();
    let axum::extract::Query(q) =
        axum::extract::Query::<EventPageQuery>::try_from_uri(&uri).unwrap();
    ListPoolSwapsRequest::parse(VALID_ADDR.to_string(), q).unwrap();
}
