//! Unit tests for `ListTopPoolsRequest::parse`.
//!
//! The metric is enforced by serde at deserialization (not retested here);
//! these cover the `limit` range validation and the wire → domain conversion.

use crate::http::dto::request::ListTopPoolsRequest;
use crate::http::error::ApiError;
use crate::http::query::{PoolRankMetricParam, TopPoolsQuery};
use yog_core::domain::PoolRankMetric;

fn query(limit: i64) -> TopPoolsQuery {
    TopPoolsQuery {
        metric: PoolRankMetricParam::Volume24h,
        limit,
    }
}

#[test]
fn parses_valid_query() {
    let request = ListTopPoolsRequest::parse(query(10)).expect("should parse");
    assert_eq!(request.metric(), PoolRankMetric::Volume24h);
    assert_eq!(request.limit(), 10);
}

#[test]
fn maps_tvl_metric_to_domain() {
    let request = ListTopPoolsRequest::parse(TopPoolsQuery {
        metric: PoolRankMetricParam::Tvl,
        limit: 10,
    })
    .expect("should parse");
    assert_eq!(request.metric(), PoolRankMetric::Tvl);
}

#[test]
fn accepts_limit_at_cap() {
    let request = ListTopPoolsRequest::parse(query(20)).expect("limit at cap should parse");
    assert_eq!(request.limit(), 20);
}

#[test]
fn rejects_limit_zero() {
    assert!(matches!(
        ListTopPoolsRequest::parse(query(0)),
        Err(ApiError::BadRequest(_))
    ));
}

#[test]
fn rejects_limit_over_cap() {
    assert!(matches!(
        ListTopPoolsRequest::parse(query(21)),
        Err(ApiError::BadRequest(_))
    ));
}
