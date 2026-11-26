//! Persistence contracts of the `signals` hypertable; detectors never write.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{
    PageDirection, PagePosition, RepositoryResult,
    domain::{Severity, Signal, SignalCursor, SignalFilter, SignalRecord},
};

/// Read/write contract for emitted signals.
#[async_trait]
pub trait SignalRepository: Send + Sync {
    /// Append a batch of signals in one round-trip; an empty slice is a no-op.
    async fn insert_batch(&self, signals: &[Signal]) -> RepositoryResult<()>;

    /// The latest severity per pool emitted by `detector` since `since`: the
    /// engine's suppression state. Pools without a signal are absent.
    async fn latest_severity_by_pool(
        &self,
        detector: &str,
        since: DateTime<Utc>,
    ) -> RepositoryResult<HashMap<Pubkey, Severity>>;
}

/// Read contract for the signal feed, the api's lens.
#[async_trait]
pub trait SignalFeed: Send + Sync {
    /// Paginate the feed kept by `filter`, newest first.
    /// The returned cursors chain the pages; `limit` may be capped.
    async fn list(
        &self,
        filter: SignalFilter,
        cursor: Option<SignalCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<SignalRecord>>;

    /// The highest id among the signals triggered within `lookback`, `None` without one.
    async fn live_tip(&self, lookback: Duration) -> RepositoryResult<Option<i64>>;

    /// Signals triggered within `lookback` with an id above `after_id`, by id, capped at `limit`.
    /// ⚠️ By id, not by time: a signal stamped before another but inserted after still comes.
    async fn live_after(
        &self,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<SignalRecord>>;
}
