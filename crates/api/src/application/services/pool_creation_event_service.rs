//! The pools' creations, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, PoolCreationEvent, PoolCreationEventLookup, Protocol, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct PoolCreationEventService {
    events: Arc<dyn PoolCreationEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl PoolCreationEventService {
    pub(crate) fn new(events: Arc<dyn PoolCreationEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    /// `protocol`'s creations (every protocol's for `None`) over `window`, ending now on the
    /// first page; a later page keeps the start its cursor carries, so the window does not slide
    /// under the pages.
    pub(crate) async fn list_events(
        &self,
        protocol: Option<Protocol>,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<PoolCreationEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self
            .events
            .list_events(protocol, from, cursor, limit)
            .await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/pool_creation_event_service_tests.rs"]
mod tests;
