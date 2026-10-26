//! Request DTO for the pools: `GET /api/pools`, `/api/protocols/{protocol}/pools` and
//! `/api/tokens/{mint}/pools`.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{PoolCursor, PoolScope};

use crate::http::{cursor::decode_pool_cursor, error::ApiError};

/// Forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct PoolsParams {
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListPoolsRequest {
    pub(crate) scope: PoolScope,
    pub(crate) cursor: Option<PoolCursor>,
    pub(crate) limit: i64,
}

impl ListPoolsRequest {
    pub(crate) fn parse(params: PoolsParams, scope: PoolScope) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw).and_then(|raw| decode_pool_cursor(raw, scope))
            })
            .transpose()?;
        Ok(Self {
            scope,
            cursor,
            limit: params.limit,
        })
    }
}
