use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// Where the announcement's window stands at the request: not begun, open, or ended.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum AnnouncementStatusResponse {
    Upcoming,
    Open,
    Closed,
}

impl From<domain::AnnouncementStatus> for AnnouncementStatusResponse {
    fn from(status: domain::AnnouncementStatus) -> Self {
        match status {
            domain::AnnouncementStatus::Upcoming => Self::Upcoming,
            domain::AnnouncementStatus::Open => Self::Open,
            domain::AnnouncementStatus::Closed => Self::Closed,
        }
    }
}
