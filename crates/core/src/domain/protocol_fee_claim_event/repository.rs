use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::ProtocolFeeClaimEvent;
use crate::domain::{EventCursor, EventScope};
use crate::{Page, RepositoryResult};

#[async_trait]
pub trait ProtocolFeeClaimEventLookup: Send + Sync {
    /// A page of `scope`'s events from `from` on, the most recent first, ties by signature then
    /// event index descending, after `cursor`'s key; forward only. `cursor` is of `scope`.
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<ProtocolFeeClaimEvent>>;
}

crate::domain::event_tables! {
    /// The tables the protocol fee claims are written in, one per protocol and event kind.
    ProtocolFeeClaimTable {
        MeteoraDammV2 => (MeteoraDammV2, "meteora_damm_v2_claim_protocol_fee_events"),
    }
}
