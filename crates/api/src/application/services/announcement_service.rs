//! Application service for operator announcements.
//!
//! Thin by design: the active-window rule lives in the repository
//! query (the window is data, not business logic), ordering is part of
//! the `AnnouncementLookup` contract, and the dismiss logic is a
//! client concern (cookie). The service's one responsibility is
//! anchoring the read to the request clock.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    RepositoryError,
    domain::{Announcement, AnnouncementLookup},
};

/// Application service for announcement queries.
pub(crate) struct AnnouncementService {
    announcement_repo: Arc<dyn AnnouncementLookup>,
}

impl AnnouncementService {
    pub(crate) fn new(announcement_repo: Arc<dyn AnnouncementLookup>) -> Self {
        Self { announcement_repo }
    }

    /// List the announcements currently in their display window, most
    /// severe first then most recent (the repository contract's ordering).
    pub(crate) async fn list_active(&self) -> Result<Vec<Announcement>, RepositoryError> {
        self.announcement_repo.list_active(Utc::now()).await
    }
}

#[cfg(test)]
#[path = "tests/announcement_service_tests.rs"]
mod tests;
