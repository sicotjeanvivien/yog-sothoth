use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::LiquidityMoveOrder;

/// `amount`, the moves worth the most first, or `at`, the most recent first.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SortParam {
    #[default]
    Amount,
    At,
}

impl From<SortParam> for LiquidityMoveOrder {
    fn from(value: SortParam) -> Self {
        match value {
            SortParam::Amount => LiquidityMoveOrder::Amount,
            SortParam::At => LiquidityMoveOrder::At,
        }
    }
}
