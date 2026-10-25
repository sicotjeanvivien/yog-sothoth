use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::AnnouncementStatus;

/// Where an announcement's window stands now: not begun, open, or ended.
#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum AnnouncementStatusParam {
    Upcoming,
    Open,
    Closed,
}

impl From<AnnouncementStatusParam> for AnnouncementStatus {
    fn from(value: AnnouncementStatusParam) -> Self {
        match value {
            AnnouncementStatusParam::Upcoming => AnnouncementStatus::Upcoming,
            AnnouncementStatusParam::Open => AnnouncementStatus::Open,
            AnnouncementStatusParam::Closed => AnnouncementStatus::Closed,
        }
    }
}
