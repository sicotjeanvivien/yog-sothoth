//! The rewards the positions claimed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, RewardClaimEvent, RewardClaimEventLookup, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct RewardClaimEventService {
    events: Arc<dyn RewardClaimEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl RewardClaimEventService {
    pub(crate) fn new(events: Arc<dyn RewardClaimEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<RewardClaimEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self.events.list_events(scope, from, cursor, limit).await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/reward_claim_event_service_tests.rs"]
mod tests;
