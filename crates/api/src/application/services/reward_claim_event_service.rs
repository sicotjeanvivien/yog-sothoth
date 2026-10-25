//! The rewards the positions claimed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, RewardClaimEvent, RewardClaimEventLookup, Window},
};

pub(crate) struct RewardClaimEventService {
    events: Arc<dyn RewardClaimEventLookup>,
}

impl RewardClaimEventService {
    pub(crate) fn new(events: Arc<dyn RewardClaimEventLookup>) -> Self {
        Self { events }
    }

    /// `scope`'s events over `window`, ending now on the first page; a later page keeps the
    /// start its cursor carries, so the window does not slide under the pages.
    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RewardClaimEvent>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        self.events.list_events(scope, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/reward_claim_event_service_tests.rs"]
mod tests;
