use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// What an announcement is about.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnnouncementKindResponse {
    Maintenance,
    Incident,
    Release,
    Beta,
}

impl From<domain::AnnouncementKind> for AnnouncementKindResponse {
    fn from(kind: domain::AnnouncementKind) -> Self {
        match kind {
            domain::AnnouncementKind::Maintenance => Self::Maintenance,
            domain::AnnouncementKind::Incident => Self::Incident,
            domain::AnnouncementKind::Release => Self::Release,
            domain::AnnouncementKind::Beta => Self::Beta,
        }
    }
}
