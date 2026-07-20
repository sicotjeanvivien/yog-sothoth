//! Announcement read contract.
//!
//! The concrete Postgres implementation lives in the `persistence`
//! crate. There is no write-side trait: publication is an operator
//! INSERT via psql until the authenticated endpoint lands with auth
//! (v0.3).

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{RepositoryResult, domain::Announcement};

/// Consultation of the currently-active announcements — the api's lens
/// (`*Lookup` convention: point-in-time read, no pagination).
#[async_trait]
pub trait AnnouncementLookup: Send + Sync {
    /// Announcements whose display window contains `now` — most severe
    /// first, then most recent `starts_at`, bounded by a hard limit.
    ///
    /// `now` is passed in rather than read from the database clock so
    /// the contract stays deterministic and testable.
    async fn active(&self, now: DateTime<Utc>) -> RepositoryResult<Vec<Announcement>>;
}
