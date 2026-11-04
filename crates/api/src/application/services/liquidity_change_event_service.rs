//! The liquidity added to and removed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, LiquidityChangeEvent, LiquidityChangeEventLookup, Window},
};

use crate::application::{Paired, PoolPairs};

pub(crate) struct LiquidityChangeEventService {
    events: Arc<dyn LiquidityChangeEventLookup>,
    pairs: Arc<PoolPairs>,
}

impl LiquidityChangeEventService {
    pub(crate) fn new(events: Arc<dyn LiquidityChangeEventLookup>, pairs: Arc<PoolPairs>) -> Self {
        Self { events, pairs }
    }

    /// `scope`'s events over `window`, ending now on the first page; a later page keeps the
    /// start its cursor carries, so the window does not slide under the pages.
    pub(crate) async fn list_events(
        &self,
        scope: EventScope,
        window: Window,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Paired<LiquidityChangeEvent>>> {
        let from = EventCursor::window_start(cursor.as_ref(), window, Utc::now());
        let page = self.events.list_events(scope, from, cursor, limit).await?;
        self.pairs.page(page).await
    }
}

#[cfg(test)]
#[path = "tests/liquidity_change_event_service_tests.rs"]
mod tests;
