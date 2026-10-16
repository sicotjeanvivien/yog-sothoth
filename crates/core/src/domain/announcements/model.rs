//! An operator → users message, shown by the dashboard while its window is open:
//! editorial, not an observation, so nothing here touches the signal types.

use chrono::{DateTime, Utc};

/// What an announcement is about; mirrors the `announcements.kind` CHECK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementKind {
    Maintenance,
    Incident,
    Release,
    Beta,
}

impl AnnouncementKind {
    /// Stable tag of the `kind` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnouncementKind::Maintenance => "maintenance",
            AnnouncementKind::Incident => "incident",
            AnnouncementKind::Release => "release",
            AnnouncementKind::Beta => "beta",
        }
    }
}

impl std::str::FromStr for AnnouncementKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "maintenance" => Ok(AnnouncementKind::Maintenance),
            "incident" => Ok(AnnouncementKind::Incident),
            "release" => Ok(AnnouncementKind::Release),
            "beta" => Ok(AnnouncementKind::Beta),
            _ => Err(()),
        }
    }
}

/// How prominently to display an announcement; the declaration order is the order.
/// ⚠️ Not the signals' `Severity`: the shared tag names are a coincidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnnouncementSeverity {
    Info,
    Warning,
    Critical,
}

impl AnnouncementSeverity {
    /// Stable tag of the `severity` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnouncementSeverity::Info => "info",
            AnnouncementSeverity::Warning => "warning",
            AnnouncementSeverity::Critical => "critical",
        }
    }
}

impl std::str::FromStr for AnnouncementSeverity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "info" => Ok(AnnouncementSeverity::Info),
            "warning" => Ok(AnnouncementSeverity::Warning),
            "critical" => Ok(AnnouncementSeverity::Critical),
            _ => Err(()),
        }
    }
}

/// One row of the `announcements` table; the domain has no write path.
#[derive(Debug, Clone, PartialEq)]
pub struct Announcement {
    /// Storage-assigned identity — the dismiss key on the web.
    pub id: i64,
    /// What the announcement is about (label chip on the web).
    pub kind: AnnouncementKind,
    /// How prominently to display it.
    pub severity: AnnouncementSeverity,
    /// Free operator text, in English.
    pub message: String,
    /// Optional target, e.g. `/changelog#v0.1.0` for a release.
    pub link_url: Option<String>,
    /// Start of the display window.
    pub starts_at: DateTime<Utc>,
    /// End of the display window; `None` = until the operator closes it.
    pub ends_at: Option<DateTime<Utc>>,
}
