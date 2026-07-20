//! Response DTO for `GET /api/announcements/active`.
//!
//! Kept separate from the domain type, like every other `*Response` in
//! this module — the domain model never leaks into the JSON wire shape.

use chrono::{DateTime, Utc};
use serde::Serialize;

use yog_core::domain::Announcement;

/// One active operator announcement, as displayed by the web banner.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnnouncementResponse {
    /// Storage identity — the client's dismiss-cookie key.
    id: i64,

    /// What it is about: "maintenance" | "incident" | "release" | "beta".
    kind: String,

    /// Display prominence: "info" | "warning" | "critical".
    severity: String,

    /// Free operator text (English, v1 decision).
    message: String,

    /// Optional target, e.g. `/changelog#v0.1.1`. `null` when absent.
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
            kind: a.kind.as_str().to_string(),
            severity: a.severity.as_str().to_string(),
            message: a.message,
            link_url: a.link_url,
            starts_at: a.starts_at,
            ends_at: a.ends_at,
        }
    }
}
