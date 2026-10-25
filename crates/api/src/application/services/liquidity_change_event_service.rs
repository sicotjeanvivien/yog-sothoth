//! The liquidity added to and removed from the pools, read live by scope over a window.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{
    Page, RepositoryResult,
    domain::{EventCursor, EventScope, LiquidityChangeEvent, LiquidityChangeEventLookup, Window},
};

pub(crate) struct LiquidityChangeEventService {
    events: Arc<dyn LiquidityChangeEventLookup>,
}

impl LiquidityChangeEventService {
    pub(crate) fn new(events: Arc<dyn LiquidityChangeEventLookup>) -> Self {
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
    ) -> RepositoryResult<Page<LiquidityChangeEvent>> {
        let from = cursor
            .as_ref()
            .map_or_else(|| Utc::now() - window.duration(), |c| c.from);
        self.events.list_events(scope, from, cursor, limit).await
    }
}
