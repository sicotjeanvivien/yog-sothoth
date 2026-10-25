//! Request DTO for `GET /api/pools/{pool}/liquidity-moves`.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{LiquidityMoveCursor, LiquidityMoveOrder, Window};

use super::window_param::WindowParam;
use crate::http::{cursor::decode_liquidity_move_cursor, error::ApiError};

mod sort_param;

use sort_param::SortParam;

/// A window, the order, and forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct PoolLiquidityMovesParams {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[serde(default)]
    #[param(inline)]
    sort: SortParam,
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListPoolLiquidityMovesRequest {
    pub(crate) window: Window,
    pub(crate) order: LiquidityMoveOrder,
    pub(crate) cursor: Option<LiquidityMoveCursor>,
    pub(crate) limit: i64,
}

impl ListPoolLiquidityMovesRequest {
    pub(crate) fn parse(params: PoolLiquidityMovesParams) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let order = params.sort.into();
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_liquidity_move_cursor(raw, order))
            })
            .transpose()?;
        Ok(Self {
            window: params.window.into(),
            order,
            cursor,
            limit: params.limit,
        })
    }
}
