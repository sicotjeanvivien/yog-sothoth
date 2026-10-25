use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{
        Announcement, AnnouncementEntry, AnnouncementKind, AnnouncementSeverity, AnnouncementStatus,
    },
};

/// Row shape of `announcements`. A tag outside the CHECK-constrained sets is an
/// integrity error.
pub(super) struct AnnouncementRow {
    pub(super) id: i64,
    pub(super) kind: String,
    pub(super) severity: String,
    pub(super) message: String,
    pub(super) link_url: Option<String>,
    pub(super) starts_at: DateTime<Utc>,
    pub(super) ends_at: Option<DateTime<Utc>>,
}

impl TryFrom<AnnouncementRow> for Announcement {
    type Error = RepositoryError;

    fn try_from(row: AnnouncementRow) -> Result<Self, Self::Error> {
        let kind: AnnouncementKind = row
            .kind
            .parse()
            .map_err(|()| RepositoryError::Integrity(format!("invalid kind: {}", row.kind)))?;
        let severity: AnnouncementSeverity = row.severity.parse().map_err(|()| {
            RepositoryError::Integrity(format!("invalid severity: {}", row.severity))
        })?;

        Ok(Announcement {
            id: row.id,
            kind,
            severity,
            message: row.message,
            link_url: row.link_url,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
    }
}

/// An announcement of the list, with the status the query computed for it.
pub(super) struct AnnouncementEntryRow {
    pub(super) id: i64,
    pub(super) kind: String,
    pub(super) severity: String,
    pub(super) message: String,
    pub(super) link_url: Option<String>,
    pub(super) starts_at: DateTime<Utc>,
    pub(super) ends_at: Option<DateTime<Utc>>,
    pub(super) status: String,
}

impl TryFrom<AnnouncementEntryRow> for AnnouncementEntry {
    type Error = RepositoryError;

    fn try_from(row: AnnouncementEntryRow) -> Result<Self, Self::Error> {
        let status: AnnouncementStatus = row
            .status
            .parse()
            .map_err(|()| RepositoryError::Integrity(format!("invalid status: {}", row.status)))?;
        let announcement = Announcement::try_from(AnnouncementRow {
            id: row.id,
            kind: row.kind,
            severity: row.severity,
            message: row.message,
            link_url: row.link_url,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })?;
        Ok(AnnouncementEntry {
            announcement,
            status,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
