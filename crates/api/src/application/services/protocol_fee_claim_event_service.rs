//! The protocol fees claimed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, ProtocolFeeClaimEvent, ProtocolFeeClaimEventLookup, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct ProtocolFeeClaimEventService {
    events: Arc<dyn ProtocolFeeClaimEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl ProtocolFeeClaimEventService {
    pub(crate) fn new(events: Arc<dyn ProtocolFeeClaimEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<ProtocolFeeClaimEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self.events.list_events(scope, from, cursor, limit).await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/protocol_fee_claim_event_service_tests.rs"]
mod tests;
