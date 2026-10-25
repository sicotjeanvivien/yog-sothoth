use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::SwapEvent;
use crate::domain::{EventCursor, EventScope};
use crate::{Page, RepositoryResult};

#[async_trait]
pub trait SwapEventLookup: Send + Sync {
    /// A page of `scope`'s events from `from` on, the most recent first, ties by signature then
    /// event index descending, after `cursor`'s key; forward only. `cursor` is of `scope`.
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<SwapEvent>>;
}
