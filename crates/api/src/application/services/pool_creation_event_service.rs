//! The pools' creations, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, PoolCreationEvent, PoolCreationEventLookup, Protocol, Window},
};

pub(crate) struct PoolCreationEventService {
    events: Arc<dyn PoolCreationEventLookup>,
}

impl PoolCreationEventService {
    pub(crate) fn new(events: Arc<dyn PoolCreationEventLookup>) -> Self {
        Self { events }
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
    ) -> RepositoryResult<Page<PoolCreationEvent>> {
        let from = cursor
            .as_ref()
            .map_or_else(|| Utc::now() - window.duration(), |c| c.from);
        self.events.list_events(protocol, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/pool_creation_event_service_tests.rs"]
mod tests;
