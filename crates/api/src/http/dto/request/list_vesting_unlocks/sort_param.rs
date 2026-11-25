use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::VestingUnlockOrder;

/// `time`, the soonest first, or `amount`, the largest first, an unvalued release last.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum SortParam {
    #[default]
    Time,
    Amount,
}

impl From<SortParam> for VestingUnlockOrder {
    fn from(value: SortParam) -> Self {
        match value {
            SortParam::Time => VestingUnlockOrder::Time,
            SortParam::Amount => VestingUnlockOrder::Amount,
        }
    }
}
