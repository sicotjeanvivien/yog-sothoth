//! The liquidity brought by the pools' creation, added and removed, valued; the sums skip the
//! moves not valued, which `moves - movesValued` counts. The fields of every liquidity row.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::LiquidityMoveSums;

use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityMoveSumsResponse {
    #[schema(value_type = UnsignedDecimal)]
    created_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    added_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    removed_usd: Decimal,
    #[schema(value_type = Count)]
    moves: i64,
    #[schema(value_type = Count)]
    moves_valued: i64,
}

impl From<LiquidityMoveSums> for LiquidityMoveSumsResponse {
    fn from(s: LiquidityMoveSums) -> Self {
        Self {
            created_usd: s.created_usd,
            added_usd: s.added_usd,
            removed_usd: s.removed_usd,
            moves: s.moves,
            moves_valued: s.moves_valued,
        }
    }
}
