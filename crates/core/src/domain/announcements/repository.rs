//! Announcement read contract; there is no write side, publication is an operator INSERT.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{RepositoryResult, domain::Announcement};

/// The currently active announcements, the api's lens.
#[async_trait]
pub trait AnnouncementLookup: Send + Sync {
    /// The announcements whose window contains `now`, most severe then most
    /// recent first, bounded by a hard limit.
    async fn list_active(&self, now: DateTime<Utc>) -> RepositoryResult<Vec<Announcement>>;
}
