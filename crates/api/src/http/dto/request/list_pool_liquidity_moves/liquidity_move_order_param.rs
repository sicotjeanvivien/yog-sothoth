use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::LiquidityMoveOrder;

/// `amount`, the moves worth the most first, or `at`, the most recent first.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LiquidityMoveOrderParam {
    #[default]
    Amount,
    At,
}

impl From<LiquidityMoveOrderParam> for LiquidityMoveOrder {
    fn from(value: LiquidityMoveOrderParam) -> Self {
        match value {
            LiquidityMoveOrderParam::Amount => LiquidityMoveOrder::Amount,
            LiquidityMoveOrderParam::At => LiquidityMoveOrder::At,
        }
    }
}
