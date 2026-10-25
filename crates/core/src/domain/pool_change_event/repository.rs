use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::PoolChangeEvent;
use crate::domain::{EventCursor, EventScope};
use crate::{Page, RepositoryResult};

#[async_trait]
pub trait PoolChangeEventLookup: Send + Sync {
    /// A page of `scope`'s changes from `from` on, every kind merged, the most recent first, ties
    /// by signature then event index descending, after `cursor`'s key; forward only.
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolChangeEvent>>;
}
