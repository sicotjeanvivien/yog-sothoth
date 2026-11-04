//! The live reads of an event collection, table by table.
//! ⚠️ An id is unique in its table only: a live read keeps its place per table, never per
//! collection nor across protocols.

use async_trait::async_trait;
use chrono::Duration;

use crate::RepositoryResult;
use crate::domain::Protocol;

/// One table a collection is written in; declared by `event_tables!`, never by hand.
pub trait EventTable: Copy + Send + Sync + 'static {
    /// Every table of the collection.
    const ALL: &'static [Self];

    /// Its name in the database, the logs and a live cursor: unique across collections.
    fn name(self) -> &'static str;

    fn protocol(self) -> Protocol;
}

/// A collection's tables, their names and protocols from one list.
/// ⚠️ One list: a table declared outside `ALL` would compile and never be read live.
macro_rules! event_tables {
    ($(#[$meta:meta])* $enum:ident { $($table:ident => ($protocol:ident, $name:literal)),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $enum {
            $($table),+
        }

        impl $crate::domain::EventTable for $enum {
            const ALL: &'static [Self] = &[$(Self::$table),+];

            fn name(self) -> &'static str {
                match self {
                    $(Self::$table => $name),+
                }
            }

            fn protocol(self) -> $crate::domain::Protocol {
                match self {
                    $(Self::$table => $crate::domain::Protocol::$protocol),+
                }
            }
        }
    };
}
pub(crate) use event_tables;

/// An event read live, with its id in its table.
#[derive(Debug, Clone)]
pub struct LiveRecord<E> {
    pub id: i64,
    pub event: E,
}

#[async_trait]
pub trait LiveEventFeed<T: EventTable, E>: Send + Sync {
    /// The highest id of `table` among its events within `lookback`, `None` without one.
    async fn live_tip(&self, table: T, lookback: Duration) -> RepositoryResult<Option<i64>>;

    /// `table`'s events within `lookback` with an id above `after_id`, by id, capped at `limit`.
    /// ⚠️ By id, not by time: an event stamped before another but inserted after still comes.
    /// A row that does not decode comes as its error, with its id: it does not hold the table.
    async fn live_after(
        &self,
        table: T,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<E>>>>;
}
