//! Response DTO for `GET /api/announcements/active`.

mod entry_response;
mod kind_response;
mod severity_response;
mod status_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::Announcement;

use crate::http::dto::schema::Count;

pub(crate) use entry_response::AnnouncementEntryResponse;
use kind_response::AnnouncementKindResponse;
use severity_response::AnnouncementSeverityResponse;

/// One active operator announcement, as displayed by the web banner.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnnouncementResponse {
    /// Storage identity — the client's dismiss-cookie key.
    #[schema(value_type = Count)]
    id: i64,

    /// What it is about: "maintenance" | "incident" | "release" | "beta".
    kind: AnnouncementKindResponse,

    /// Display prominence: "info" | "warning" | "critical".
    severity: AnnouncementSeverityResponse,

    /// Free operator text (English, v1 decision).
    message: String,

    /// Optional target, e.g. `/changelog#v0.1.0`. `null` when absent.
    link_url: Option<String>,

    /// Start of the display window.
    starts_at: DateTime<Utc>,

    /// End of the display window; `null` = open-ended.
    ends_at: Option<DateTime<Utc>>,
}

impl From<Announcement> for AnnouncementResponse {
    fn from(a: Announcement) -> Self {
        Self {
            id: a.id,
            kind: a.kind.into(),
            severity: a.severity.into(),
            message: a.message,
            link_url: a.link_url,
            starts_at: a.starts_at,
            ends_at: a.ends_at,
        }
    }
}

#[cfg(test)]
#[path = "tests/announcement_tests.rs"]
mod tests;
