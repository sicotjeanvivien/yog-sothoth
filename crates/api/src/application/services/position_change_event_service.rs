//! The positions opened, closed, locked and split in the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, PositionChangeEvent, PositionChangeEventLookup, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct PositionChangeEventService {
    events: Arc<dyn PositionChangeEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl PositionChangeEventService {
    pub(crate) fn new(events: Arc<dyn PositionChangeEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<PositionChangeEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self.events.list_events(scope, from, cursor, limit).await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/position_change_event_service_tests.rs"]
mod tests;
