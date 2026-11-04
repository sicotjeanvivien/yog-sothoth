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

crate::domain::event_tables! {
    /// The tables the pool changes are written in, one per protocol and event kind.
    PoolChangeTable {
        MeteoraDammV2Status => (MeteoraDammV2, "meteora_damm_v2_set_pool_status_events"),
        MeteoraDammV2Fees => (MeteoraDammV2, "meteora_damm_v2_update_pool_fees_events"),
    }
}
