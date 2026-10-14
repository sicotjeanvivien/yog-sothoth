//! Response DTO for `GET /api/announcements/active`.
//!
//! Kept separate from the domain type, like every other `*Response` in
//! this module — the domain model never leaks into the JSON wire shape.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{self, Announcement};

use crate::http::dto::schema::Count;

/// One active operator announcement, as displayed by the web banner.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnnouncementResponse {
    /// Storage identity — the client's dismiss-cookie key.
    #[schema(value_type = Count)]
    id: i64,

    /// What it is about: "maintenance" | "incident" | "release" | "beta".
    kind: AnnouncementKind,

    /// Display prominence: "info" | "warning" | "critical".
    severity: AnnouncementSeverity,

    /// Free operator text (English, v1 decision).
    message: String,

    /// Optional target, e.g. `/changelog#v0.1.0`. `null` when absent.
    link_url: Option<String>,

    /// Start of the display window.
    starts_at: DateTime<Utc>,

    /// End of the display window; `null` = open-ended.
    ends_at: Option<DateTime<Utc>>,
}

/// What an announcement is about.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnnouncementKind {
    Maintenance,
    Incident,
    Release,
    Beta,
}

impl From<domain::AnnouncementKind> for AnnouncementKind {
    fn from(kind: domain::AnnouncementKind) -> Self {
        match kind {
            domain::AnnouncementKind::Maintenance => Self::Maintenance,
            domain::AnnouncementKind::Incident => Self::Incident,
            domain::AnnouncementKind::Release => Self::Release,
            domain::AnnouncementKind::Beta => Self::Beta,
        }
    }
}

/// How prominently an announcement is shown.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnnouncementSeverity {
    Info,
    Warning,
    Critical,
}

impl From<domain::AnnouncementSeverity> for AnnouncementSeverity {
    fn from(severity: domain::AnnouncementSeverity) -> Self {
        match severity {
            domain::AnnouncementSeverity::Info => Self::Info,
            domain::AnnouncementSeverity::Warning => Self::Warning,
            domain::AnnouncementSeverity::Critical => Self::Critical,
        }
    }
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
