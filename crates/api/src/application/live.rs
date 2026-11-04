//! The pollers feeding `/api/live`: one task per source walks its table by id, reads only while
//! someone listens, and broadcasts each new item once to every connection. Polling, not
//! LISTEN/NOTIFY: the table stays the only contract. ⚠️ Nothing is replayed: a page loads the
//! history through the source's collection.

use std::future::Future;

use yog_core::RepositoryResult;

mod poller;
mod signals_source;
mod watermark;

pub(crate) use poller::LivePoller;
pub(crate) use signals_source::SignalsSource;

/// Capacity of each source's broadcast channel. A connection lagging further gets `Lagged` and
/// closes: the page reconnects and refetches rather than silently missing items.
pub(crate) const LIVE_CHANNEL_CAPACITY: usize = 256;

/// A table `/api/live` streams: read by id, then made into the items it broadcasts.
pub(crate) trait LiveSource: Send + Sync + 'static {
    type Record: Send;
    type Item: Send + Sync + 'static;

    /// Its name in the logs.
    const NAME: &'static str;

    /// The highest id there is, `None` on an empty table.
    fn tip(&self) -> impl Future<Output = RepositoryResult<Option<i64>>> + Send;

    /// Records with an id above `after_id`, by id, at most `limit`.
    fn after(
        &self,
        after_id: i64,
        limit: i64,
    ) -> impl Future<Output = RepositoryResult<Vec<Self::Record>>> + Send;

    fn id_of(record: &Self::Record) -> i64;

    /// The items to broadcast, one per record and in its order; it fails nothing.
    fn prepare(&self, records: Vec<Self::Record>) -> impl Future<Output = Vec<Self::Item>> + Send;
}
