use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::PositionChangeEvent;
use crate::domain::{EventCursor, EventScope};
use crate::{Page, RepositoryResult};

#[async_trait]
pub trait PositionChangeEventLookup: Send + Sync {
    /// A page of `scope`'s changes from `from` on, every kind merged, the most recent first, ties
    /// by signature then event index descending, after `cursor`'s key; forward only.
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PositionChangeEvent>>;
}

crate::domain::event_tables! {
    /// The tables the position changes are written in, one per protocol and event kind.
    PositionChangeTable {
        MeteoraDammV2Open => (MeteoraDammV2, "meteora_damm_v2_create_position_events"),
        MeteoraDammV2Close => (MeteoraDammV2, "meteora_damm_v2_close_position_events"),
        MeteoraDammV2VestingLock => (MeteoraDammV2, "meteora_damm_v2_lock_position_events"),
        MeteoraDammV2PermanentLock => (MeteoraDammV2, "meteora_damm_v2_permanent_lock_position_events"),
        MeteoraDammV2Split => (MeteoraDammV2, "meteora_damm_v2_split_position_events"),
    }
}
