//! The rewards of the pools set up, funded, changed and withdrawn, read live by scope.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, RewardChangeEvent, RewardChangeEventLookup, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct RewardChangeEventService {
    events: Arc<dyn RewardChangeEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl RewardChangeEventService {
    pub(crate) fn new(events: Arc<dyn RewardChangeEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<RewardChangeEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self.events.list_events(scope, from, cursor, limit).await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/reward_change_event_service_tests.rs"]
mod tests;
