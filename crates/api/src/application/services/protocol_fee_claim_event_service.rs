//! The protocol fees claimed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, ProtocolFeeClaimEvent, ProtocolFeeClaimEventLookup, Window},
};

pub(crate) struct ProtocolFeeClaimEventService {
    events: Arc<dyn ProtocolFeeClaimEventLookup>,
}

impl ProtocolFeeClaimEventService {
    pub(crate) fn new(events: Arc<dyn ProtocolFeeClaimEventLookup>) -> Self {
        Self { events }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<ProtocolFeeClaimEvent>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        self.events.list_events(scope, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/protocol_fee_claim_event_service_tests.rs"]
mod tests;
