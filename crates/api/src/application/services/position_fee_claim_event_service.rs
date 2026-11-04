//! The fees the positions claimed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, PositionFeeClaimEvent, PositionFeeClaimEventLookup, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct PositionFeeClaimEventService {
    events: Arc<dyn PositionFeeClaimEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl PositionFeeClaimEventService {
    pub(crate) fn new(events: Arc<dyn PositionFeeClaimEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<PositionFeeClaimEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self.events.list_events(scope, from, cursor, limit).await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/position_fee_claim_event_service_tests.rs"]
mod tests;
