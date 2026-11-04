use std::sync::Arc;

use tracing::warn;
use yog_core::RepositoryResult;
use yog_core::domain::{
    EventTable, LiquidityChangeEvent, LiveEventFeed, LiveRecord, PoolChangeEvent,
    PoolCreationEvent, PositionChangeEvent, PositionFeeClaimEvent, Protocol, ProtocolFeeClaimEvent,
    RewardChangeEvent, RewardClaimEvent,
};

use super::LiveSource;

/// How far back the events are read live, in seconds.
/// ⚠️ An event stamped further back when inserted is never streamed: the bound is what keeps
/// the read off the compressed chunks, and its collection still lists it.
const EVENTS_LOOKBACK_SECS: i64 = 86_400;

/// An event as it is broadcast: its table and its id in it, which place it in a live cursor.
#[derive(Debug)]
pub(crate) struct LiveEvent<T, E> {
    pub(crate) table: T,
    pub(crate) record: LiveRecord<E>,
}

/// An event of a collection, as a protocol's connection filters it.
pub(crate) trait LiveItem {
    fn protocol(&self) -> Protocol;
}

macro_rules! live_items {
    ($($event:ty),+) => {
        $(impl LiveItem for $event {
            fn protocol(&self) -> Protocol {
                <$event>::protocol(self)
            }
        })+
    };
}

live_items!(
    LiquidityChangeEvent,
    PoolCreationEvent,
    PositionChangeEvent,
    PositionFeeClaimEvent,
    ProtocolFeeClaimEvent,
    RewardClaimEvent,
    RewardChangeEvent,
    PoolChangeEvent
);

/// One table of an event collection, its events broadcast as they are read.
pub(crate) struct EventSource<T, E> {
    feed: Arc<dyn LiveEventFeed<T, E>>,
    table: T,
}

impl<T, E> EventSource<T, E> {
    pub(crate) fn new(feed: Arc<dyn LiveEventFeed<T, E>>, table: T) -> Self {
        Self { feed, table }
    }
}

impl<T: EventTable, E: Send + Sync + 'static> LiveSource for EventSource<T, E> {
    type Record = LiveRecord<RepositoryResult<E>>;
    type Item = LiveEvent<T, E>;

    fn name(&self) -> &'static str {
        self.table.name()
    }

    async fn tip(&self) -> RepositoryResult<Option<i64>> {
        self.feed
            .live_tip(self.table, chrono::Duration::seconds(EVENTS_LOOKBACK_SECS))
            .await
    }

    async fn after(
        &self,
        after_id: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<E>>>> {
        self.feed
            .live_after(
                self.table,
                after_id,
                chrono::Duration::seconds(EVENTS_LOOKBACK_SECS),
                limit,
            )
            .await
    }

    fn id_of(record: &LiveRecord<RepositoryResult<E>>) -> i64 {
        record.id
    }

    /// A row that did not decode is logged and passed over: its id is counted as delivered.
    async fn prepare(&self, records: Vec<LiveRecord<RepositoryResult<E>>>) -> Vec<LiveEvent<T, E>> {
        records
            .into_iter()
            .filter_map(|LiveRecord { id, event }| match event {
                Ok(event) => Some(LiveEvent {
                    table: self.table,
                    record: LiveRecord { id, event },
                }),
                Err(e) => {
                    warn!(table = self.table.name(), id, error = %e, "live event undecodable — skipped");
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "event_source_tests.rs"]
mod tests;
