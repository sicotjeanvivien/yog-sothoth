use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::PoolCreationEvent;
use crate::domain::{EventCursor, Protocol};
use crate::{Page, RepositoryResult};

#[async_trait]
pub trait PoolCreationEventLookup: Send + Sync {
    /// A page of `protocol`'s creations (every protocol's for `None`) from `from` on, the most
    /// recent first, ties by signature then event index descending, after `cursor`'s key;
    /// forward only. A pool has a single creation: no list reads one pool's.
    async fn list_events(
        &self,
        protocol: Option<Protocol>,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolCreationEvent>>;
}
