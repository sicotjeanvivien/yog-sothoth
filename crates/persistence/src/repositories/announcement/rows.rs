use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{Announcement, AnnouncementKind, AnnouncementSeverity},
};

/// Row shape for reading `announcements`.
///
/// A thin sqlx-facing struct kept separate from the domain model: it
/// holds the raw TEXT tags, parsed back into their enums in the
/// `TryFrom` impl below. A tag outside the CHECK-constrained sets can
/// only mean schema/code drift — surfaced as an integrity error.
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

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
