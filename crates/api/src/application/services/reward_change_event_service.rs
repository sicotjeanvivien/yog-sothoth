//! The rewards of the pools set up, funded, changed and withdrawn, read live by scope.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, RewardChangeEvent, RewardChangeEventLookup, Window},
};

pub(crate) struct RewardChangeEventService {
    events: Arc<dyn RewardChangeEventLookup>,
}

impl RewardChangeEventService {
    pub(crate) fn new(events: Arc<dyn RewardChangeEventLookup>) -> Self {
        Self { events }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RewardChangeEvent>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        self.events.list_events(scope, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/reward_change_event_service_tests.rs"]
mod tests;
