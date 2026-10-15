use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// How prominently an announcement is shown.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum AnnouncementSeverityResponse {
    Info,
    Warning,
    Critical,
}

impl From<domain::AnnouncementSeverity> for AnnouncementSeverityResponse {
    fn from(severity: domain::AnnouncementSeverity) -> Self {
        match severity {
            domain::AnnouncementSeverity::Info => Self::Info,
            domain::AnnouncementSeverity::Warning => Self::Warning,
            domain::AnnouncementSeverity::Critical => Self::Critical,
        }
    }
}
