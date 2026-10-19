use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::PagePosition;

#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum PagePositionParam {
    First,
    Last,
}

impl From<PagePositionParam> for PagePosition {
    fn from(value: PagePositionParam) -> Self {
        match value {
            PagePositionParam::First => PagePosition::First,
            PagePositionParam::Last => PagePosition::Last,
        }
    }
}
