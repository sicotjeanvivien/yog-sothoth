use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::RotationRunOrder;

/// `volume`, the runs with the most volume first, or `at`, the latest start first.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum RotationRunSortParam {
    #[default]
    Volume,
    At,
}

impl From<RotationRunSortParam> for RotationRunOrder {
    fn from(value: RotationRunSortParam) -> Self {
        match value {
            RotationRunSortParam::Volume => RotationRunOrder::Volume,
            RotationRunSortParam::At => RotationRunOrder::At,
        }
    }
}
