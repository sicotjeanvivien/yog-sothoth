use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::FreshnessStatus;

/// How recent the last indexed event is.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FreshnessStatusResponse {
    Live,
    Delayed,
    Stale,
}

impl From<FreshnessStatus> for FreshnessStatusResponse {
    fn from(status: FreshnessStatus) -> Self {
        match status {
            FreshnessStatus::Live => Self::Live,
            FreshnessStatus::Delayed => Self::Delayed,
            FreshnessStatus::Stale => Self::Stale,
        }
    }
}
