//! Unit tests for `ListPoolsRequest::parse`.
//!
//! These tests verify the orchestration of the validation pipeline
//! and the wire → domain conversion. The individual validation rules
//! have their own dedicated tests in `http/query` and `http/cursor`
//! and are not retested here.

use crate::http::dto::request::ListPoolsRequest;
use crate::http::dto::request::test_common::valid_page_query;
use crate::http::error::ApiError;
use crate::http::query::{PageDirectionParam, PagePositionParam, PoolSortParam};
use yog_core::{PageDirection, PoolSort};

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn parses_valid_baseline_query() {
    let request = ListPoolsRequest::parse(valid_page_query()).expect("should parse");
    let params = request.into_params();
    // The baseline query carries no cursor, no position, no search.
    assert!(params.cursor.is_none());
    assert!(params.position.is_none());
    assert!(params.search.is_none());
    assert_eq!(params.limit, 50);
}

#[test]
fn preserves_dir_next() {
    let mut q = valid_page_query();
    q.dir = PageDirectionParam::Next;
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.direction, PageDirection::Next);
}

#[test]
fn preserves_dir_prev() {
    let mut q = valid_page_query();
    q.dir = PageDirectionParam::Prev;
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.direction, PageDirection::Prev);
}

#[test]
fn preserves_position_first() {
    let mut q = valid_page_query();
    q.position = Some(PagePositionParam::First);
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert!(params.position.is_some());
}

#[test]
fn preserves_limit_within_bounds() {
    let mut q = valid_page_query();
    q.limit = 75;
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.limit, 75);
}

#[test]
fn preserves_default_sort() {
    let q = valid_page_query();
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    // The default sort is whatever the wire default maps to.
    // We only check it's a stable enum variant — concrete value tested
    // in query.rs unit tests.
    let _: PoolSort = params.sort;
}

// ── Search normalisation ─────────────────────────────────────────────

#[test]
fn empty_search_normalises_to_none() {
    let mut q = valid_page_query();
    q.q = Some(String::new());
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert!(params.search.is_none());
}

#[test]
fn whitespace_search_normalises_to_none() {
    let mut q = valid_page_query();
    q.q = Some("   ".to_string());
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert!(params.search.is_none());
}

#[test]
fn meaningful_search_is_preserved() {
    let mut q = valid_page_query();
    q.q = Some("SOL".to_string());
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert_eq!(params.search.as_deref(), Some("SOL"));
}

// ── Validation errors ────────────────────────────────────────────────

#[test]
fn limit_below_minimum_rejected() {
    let mut q = valid_page_query();
    q.limit = 0;
    let err = ListPoolsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn limit_above_maximum_rejected() {
    let mut q = valid_page_query();
    q.limit = 201;
    let err = ListPoolsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn cursor_and_position_mutual_exclusion_rejected() {
    let mut q = valid_page_query();
    q.cursor = Some("anything".to_string()); // doesn't matter what — pagination check fires first
    q.position = Some(PagePositionParam::First);
    let err = ListPoolsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

#[test]
fn malformed_cursor_rejected() {
    let mut q = valid_page_query();
    q.cursor = Some("not-base64!!".to_string());
    let err = ListPoolsRequest::parse(q).unwrap_err();
    assert!(matches!(err, ApiError::BadRequest(_)));
}

// ── Cursor edge cases ────────────────────────────────────────────────

#[test]
fn empty_cursor_string_is_treated_as_none() {
    // ?cursor= (empty value) is common in URL composers — treat it
    // as cursor absent, not as a malformed cursor.
    let mut q = valid_page_query();
    q.cursor = Some(String::new());
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert!(params.cursor.is_none());
}

#[test]
fn none_cursor_stays_none() {
    let mut q = valid_page_query();
    q.cursor = None;
    let params = ListPoolsRequest::parse(q).unwrap().into_params();
    assert!(params.cursor.is_none());
}
