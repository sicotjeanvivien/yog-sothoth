//! Request DTO for `GET /api/pools/top`: serde validates the metric, only `limit` is checked here.

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
