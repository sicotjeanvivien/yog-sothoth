//! Operator announcements. Where a window stands lives in the repository query and the dismiss
//! in a client cookie: the service only anchors the read to the request clock.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryError,
    domain::{AnnouncementCursor, AnnouncementEntry, AnnouncementLookup, AnnouncementStatus},
};

pub(crate) struct AnnouncementService {
    announcement_repo: Arc<dyn AnnouncementLookup>,
}

impl AnnouncementService {
    pub(crate) fn new(announcement_repo: Arc<dyn AnnouncementLookup>) -> Self {
        Self { announcement_repo }
    }

    /// A page of every announcement, or of those of `status`, read against the request clock.
    pub(crate) async fn list_entries(
        &self,
        status: Option<AnnouncementStatus>,
        cursor: Option<AnnouncementCursor>,
        limit: i64,
    ) -> Result<Page<AnnouncementEntry>, RepositoryError> {
        self.announcement_repo
            .list_entries(status, Utc::now(), cursor, limit)
            .await
    }
}
