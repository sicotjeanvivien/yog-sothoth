//! Request DTO for `GET /api/protocols/{protocol}/liquidity-moves`.

use std::str::FromStr;

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{Protocol, Window};

use super::window_param::WindowParam;
use crate::http::error::ApiError;

/// The largest moves of a window: no cursor, a ranking read from its top.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct LiquidityMovesQuery {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListLiquidityMovesRequest {
    pub(crate) protocol: Protocol,
    pub(crate) window: Window,
    pub(crate) limit: i64,
}

impl ListLiquidityMovesRequest {
    pub(crate) fn parse(protocol: &str, query: LiquidityMovesQuery) -> Result<Self, ApiError> {
        let protocol = Protocol::from_str(protocol)
            .map_err(|_| ApiError::BadRequest(format!("unknown protocol: {protocol}")))?;
        super::validate_limit(query.limit)?;
        Ok(Self {
            protocol,
            window: query.window.into(),
            limit: query.limit,
        })
    }
}
