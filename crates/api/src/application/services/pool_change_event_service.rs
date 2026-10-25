//! The status and fee changes of the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, PoolChangeEvent, PoolChangeEventLookup, Window},
};

pub(crate) struct PoolChangeEventService {
    events: Arc<dyn PoolChangeEventLookup>,
}

impl PoolChangeEventService {
    pub(crate) fn new(events: Arc<dyn PoolChangeEventLookup>) -> Self {
        Self { events }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolChangeEvent>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        self.events.list_events(scope, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/pool_change_event_service_tests.rs"]
mod tests;
