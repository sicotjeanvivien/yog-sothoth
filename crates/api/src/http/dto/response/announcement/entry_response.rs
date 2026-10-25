use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::AnnouncementEntry;

use super::{
    kind_response::AnnouncementKindResponse, severity_response::AnnouncementSeverityResponse,
    status_response::AnnouncementStatusResponse,
};
use crate::http::dto::schema::Count;

/// An announcement of `/api/announcements`, and where its window stands at the request.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnnouncementEntryResponse {
    #[schema(value_type = Count)]
    id: i64,
    kind: AnnouncementKindResponse,
    severity: AnnouncementSeverityResponse,
    message: String,
    link_url: Option<String>,
    starts_at: DateTime<Utc>,
    /// `null` until the operator closes it.
    ends_at: Option<DateTime<Utc>>,
    status: AnnouncementStatusResponse,
}

impl From<AnnouncementEntry> for AnnouncementEntryResponse {
    fn from(entry: AnnouncementEntry) -> Self {
        let a = entry.announcement;
        Self {
            id: a.id,
            kind: a.kind.into(),
            severity: a.severity.into(),
            message: a.message,
            link_url: a.link_url,
            starts_at: a.starts_at,
            ends_at: a.ends_at,
            status: entry.status.into(),
        }
    }
}
