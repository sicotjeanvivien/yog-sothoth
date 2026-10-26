//! Announcement read contract; there is no write side, publication is an operator INSERT.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    Page, RepositoryResult,
    domain::{AnnouncementCursor, AnnouncementEntry, AnnouncementStatus},
};

/// The announcements, the api's lens.
#[async_trait]
pub trait AnnouncementLookup: Send + Sync {
    /// A page of every announcement, or of those whose window stands at `status` at `now`,
    /// the latest start first, ties by id descending, after `cursor`'s key; forward only.
    async fn list_entries(
        &self,
        status: Option<AnnouncementStatus>,
        now: DateTime<Utc>,
        cursor: Option<AnnouncementCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<AnnouncementEntry>>;
}
