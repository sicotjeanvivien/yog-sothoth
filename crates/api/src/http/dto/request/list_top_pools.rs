//! Request DTO for `GET /api/pools/top`.
//!
//! Validates the top-N ranking inputs: the metric is enforced by serde at
//! deserialization (unknown value → 400 before this runs), so only the `limit`
//! range needs checking here. By the time a `ListTopPoolsRequest` exists, both
//! fields are valid and ready for the service.

use yog_core::domain::PoolRankMetric;

use crate::http::{
    error::ApiError,
    query::{TopPoolsQuery, validate_top_limit},
};

#[derive(Debug)]
pub(crate) struct ListTopPoolsRequest {
    metric: PoolRankMetric,
    limit: i64,
}

impl ListTopPoolsRequest {
    pub(crate) fn parse(query: TopPoolsQuery) -> Result<Self, ApiError> {
        validate_top_limit(query.limit)?;
        Ok(Self {
            metric: query.metric.into(),
            limit: query.limit,
        })
    }

    pub(crate) fn metric(&self) -> PoolRankMetric {
        self.metric
    }

    pub(crate) fn limit(&self) -> i64 {
        self.limit
    }
}

#[cfg(test)]
#[path = "tests/list_top_pools_tests.rs"]
mod tests;
