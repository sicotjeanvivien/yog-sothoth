//! Announcement domain model.
//!
//! An announcement is a one-way operator → users message (maintenance,
//! incident, release, beta) displayed by the dashboard while its time
//! window is open. It is *editorial product surface*, not an observation
//! of on-chain activity — which is why nothing here touches the Signal
//! Engine types, even where the vocabulary overlaps.

use chrono::{DateTime, Utc};

/// What an announcement is about. Closed, stable set → an enum, mirrored
/// one-to-one by the `announcements.kind` CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementKind {
    Maintenance,
    Incident,
    Release,
    Beta,
}

impl AnnouncementKind {
    /// Stable snake_case tag, as persisted in the `kind` column.
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

/// How prominently an announcement should be displayed.
///
/// Deliberately **not** the Signal Engine's `Severity`: a signal severity
/// is a detector's business conclusion, with escalation semantics the
/// engine's dedup depends on; this is the operator's editorial display
/// choice. The two scales sharing three tag names today is vocabulary
/// coincidence, not concept identity — coupling them would let a future
/// Signal Engine change silently reshape product communication.
///
/// `Ord` follows the declaration order (`Info < Warning < Critical`) so
/// callers can pick the most prominent announcement to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnnouncementSeverity {
    Info,
    Warning,
    Critical,
}

impl AnnouncementSeverity {
    /// Stable snake_case tag, as persisted in the `severity` column.
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

/// One row of the `announcements` table, as read by the api.
///
/// Carries its storage `id` directly (no write-side/read-side split à la
/// `Signal`/`SignalRecord`): the domain has no write path — publication
/// is an operator INSERT via psql until the authenticated endpoint lands
/// with auth (v0.3) — and the id is what the web's dismiss cookie keys on.
#[derive(Debug, Clone, PartialEq)]
pub struct Announcement {
    /// Storage-assigned identity — the dismiss key on the web.
    pub id: i64,
    /// What the announcement is about (label chip on the web).
    pub kind: AnnouncementKind,
    /// How prominently to display it.
    pub severity: AnnouncementSeverity,
    /// Free operator text (English by decision, v1).
    pub message: String,
    /// Optional target, e.g. `/changelog#v0.1.1` for a release.
    pub link_url: Option<String>,
    /// Start of the display window.
    pub starts_at: DateTime<Utc>,
    /// End of the display window; `None` = open-ended (shown until the
    /// operator closes it).
    pub ends_at: Option<DateTime<Utc>>,
}
