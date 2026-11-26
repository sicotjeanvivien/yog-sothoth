use chrono::{DateTime, Utc};

use super::Announcement;

/// Where an announcement's window stands at an instant: not begun, open, or ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementStatus {
    Upcoming,
    Open,
    Closed,
}

impl AnnouncementStatus {
    /// Stable tag, the one the repository computes.
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnouncementStatus::Upcoming => "upcoming",
            AnnouncementStatus::Open => "open",
            AnnouncementStatus::Closed => "closed",
        }
    }
}

impl std::str::FromStr for AnnouncementStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "upcoming" => Ok(AnnouncementStatus::Upcoming),
            "open" => Ok(AnnouncementStatus::Open),
            "closed" => Ok(AnnouncementStatus::Closed),
            _ => Err(()),
        }
    }
}

/// An announcement of the list, and where its window stood at the instant it was read.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnouncementEntry {
    pub announcement: Announcement,
    pub status: AnnouncementStatus,
}

/// The key of the last announcement of a page, and the status the list is filtered on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementCursor {
    pub status: Option<AnnouncementStatus>,
    pub starts_at: DateTime<Utc>,
    pub id: i64,
}
