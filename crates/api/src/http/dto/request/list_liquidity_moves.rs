//! Request DTO for the lists of liquidity moves, under a protocol and under a pool.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{
    LiquidityMoveCursor, LiquidityMoveList, LiquidityMoveOrder, LiquidityMoveScope, Window,
};

use super::window_param::WindowParam;
use crate::http::{cursor::decode_liquidity_move_cursor, error::ApiError};

mod kind_param;
mod sort_param;

use kind_param::KindParam;
use sort_param::SortParam;

/// A window, the kind kept, the order, and forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct LiquidityMovesParams {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[param(inline)]
    kind: Option<KindParam>,
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
pub(crate) struct ListLiquidityMovesRequest {
    pub(crate) window: Window,
    pub(crate) list: LiquidityMoveList,
    pub(crate) order: LiquidityMoveOrder,
    pub(crate) cursor: Option<LiquidityMoveCursor>,
    pub(crate) limit: i64,
}

impl ListLiquidityMovesRequest {
    pub(crate) fn parse(
        params: LiquidityMovesParams,
        scope: LiquidityMoveScope,
    ) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let list = LiquidityMoveList {
            scope,
            kind: params.kind.map(Into::into),
        };
        let order = params.sort.into();
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_liquidity_move_cursor(raw, list, order))
            })
            .transpose()?;
        Ok(Self {
            window: params.window.into(),
            list,
            order,
            cursor,
            limit: params.limit,
        })
    }
}
