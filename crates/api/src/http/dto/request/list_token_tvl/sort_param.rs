use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::TokenTvlOrder;

/// `tvl`, the tokens holding the most first, or `locked`, the most locked first, an unknown
/// locked part last.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum SortParam {
    #[default]
    Tvl,
    Locked,
}

impl From<SortParam> for TokenTvlOrder {
    fn from(value: SortParam) -> Self {
        match value {
            SortParam::Tvl => TokenTvlOrder::Tvl,
            SortParam::Locked => TokenTvlOrder::Locked,
        }
    }
}
