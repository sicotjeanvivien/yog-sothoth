//! Persistence contracts of the `signals` hypertable; detectors never write.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{
    PageDirection, PagePosition, RepositoryResult,
    domain::{Severity, Signal, SignalRecord},
};

/// The last item of a page in the feed ordering, `(triggered_at, id)` descending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalCursor {
    pub triggered_at: DateTime<Utc>,
    pub id: i64,
}

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
    /// Paginate the feed, newest first; `severity` and `pool` filters combine with AND.
    async fn list(
        &self,
        severity: Option<Severity>,
        pool: Option<Pubkey>,
        cursor: Option<SignalCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<SignalRecord>>;

    /// The tip of the feed, `None` before any signal: a streaming consumer's
    /// starting watermark.
    async fn latest_cursor(&self) -> RepositoryResult<Option<SignalCursor>>;

    /// Signals strictly after `after`, oldest first, capped at `limit`.
    /// ⚠️ Exactly `limit` rows may mean more are pending: the next poll drains them.
    async fn newer_than(
        &self,
        after: &SignalCursor,
        limit: i64,
    ) -> RepositoryResult<Vec<SignalRecord>>;

    /// Per pool, the signals after `since`, newest first, capped at
    /// `per_pool_limit` each; pools without one are absent.
    async fn recent_by_pools(
        &self,
        pools: &[Pubkey],
        since: DateTime<Utc>,
        per_pool_limit: i64,
    ) -> RepositoryResult<HashMap<Pubkey, Vec<SignalRecord>>>;
}
