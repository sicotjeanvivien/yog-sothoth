//! Operator announcements. The active window lives in the repository query and the dismiss
//! in a client cookie: the service only anchors the read to the request clock.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    RepositoryError,
    domain::{Announcement, AnnouncementLookup},
};

pub(crate) struct AnnouncementService {
    announcement_repo: Arc<dyn AnnouncementLookup>,
}

impl AnnouncementService {
    pub(crate) fn new(announcement_repo: Arc<dyn AnnouncementLookup>) -> Self {
        Self { announcement_repo }
    }

    /// The announcements in their display window, most severe then most recent.
    pub(crate) async fn list_active(&self) -> Result<Vec<Announcement>, RepositoryError> {
        self.announcement_repo.list_active(Utc::now()).await
    }
}

#[cfg(test)]
#[path = "tests/announcement_service_tests.rs"]
mod tests;
