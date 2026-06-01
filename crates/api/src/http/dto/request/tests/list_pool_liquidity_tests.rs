//! Unit tests for `ListPoolLiquidityRequest::parse`.

use crate::http::dto::request::ListPoolLiquidityRequest;
use crate::http::dto::request::test_common::valid_page_query;
use crate::http::error::ApiError;
use crate::http::query::{PageDirectionParam, PagePositionParam};
use yog_core::PageDirection;

const VALID_ADDR: &str = "AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv";

#[test]
fn parses_valid_address_and_query() {
    let request =
        ListPoolLiquidityRequest::parse(VALID_ADDR.to_string(), valid_page_query()).unwrap();
    let params = request.into_params();
    assert_eq!(params.pool_address.to_string(), VALID_ADDR);
    assert!(params.cursor.is_none());
    assert_eq!(params.limit, 50);
}

#[test]
fn rejects_invalid_pool_address() {
    let err =
        ListPoolLiquidityRequest::parse("garbage".to_string(), valid_page_query()).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_invalid_limit() {
    let mut q = valid_page_query();
    q.limit = 500;
    let err = ListPoolLiquidityRequest::parse(VALID_ADDR.to_string(), q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_cursor_position_combination() {
    let mut q = valid_page_query();
    q.cursor = Some("any".to_string());
    q.position = Some(PagePositionParam::First);
    let err = ListPoolLiquidityRequest::parse(VALID_ADDR.to_string(), q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn rejects_malformed_cursor() {
    let mut q = valid_page_query();
    q.cursor = Some("not-base64!!".to_string());
    let err = ListPoolLiquidityRequest::parse(VALID_ADDR.to_string(), q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn empty_cursor_is_treated_as_none() {
    let mut q = valid_page_query();
    q.cursor = Some(String::new());
    let params = ListPoolLiquidityRequest::parse(VALID_ADDR.to_string(), q)
        .unwrap()
        .into_params();
    assert!(params.cursor.is_none());
}

#[test]
fn direction_is_threaded_through() {
    let mut q = valid_page_query();
    q.dir = PageDirectionParam::Prev;
    let params = ListPoolLiquidityRequest::parse(VALID_ADDR.to_string(), q)
        .unwrap()
        .into_params();
    assert_eq!(params.direction, PageDirection::Prev);
}
