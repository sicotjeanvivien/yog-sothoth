//! The positions opened, closed, locked and split in the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, PositionChangeEvent, PositionChangeEventLookup, Window},
};

pub(crate) struct PositionChangeEventService {
    events: Arc<dyn PositionChangeEventLookup>,
}

impl PositionChangeEventService {
    pub(crate) fn new(events: Arc<dyn PositionChangeEventLookup>) -> Self {
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
    ) -> RepositoryResult<Page<PositionChangeEvent>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        self.events.list_events(scope, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/position_change_event_service_tests.rs"]
mod tests;
