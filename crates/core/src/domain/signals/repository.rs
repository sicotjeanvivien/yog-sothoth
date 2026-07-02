//! Signal repository trait.
//!
//! Persistence contract for the `signals` hypertable. The engine calls
//! it after a detector tick to persist the returned signals; detectors
//! themselves never write. The concrete `Pg` implementation lives in
//! `yog-persistence`.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::tools::Page;
use crate::{
    PageDirection, PagePosition, RepositoryResult,
    domain::{Severity, Signal, SignalRecord},
};

/// Cursor identifying a position in the canonical signal ordering
/// (`triggered_at DESC`, `id DESC` as tiebreaker — newest first).
///
/// A cursor points to the *last item of the current page*; the next
/// page contains items strictly after this position in the ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalCursor {
    pub triggered_at: DateTime<Utc>,
    pub id: i64,
}

/// Read/write contract for emitted signals.
#[async_trait]
pub trait SignalRepository: Send + Sync {
    /// Persist a batch of freshly-detected signals in one round-trip.
    /// `signals` is append-only — each is a new row keyed by
    /// `(id, triggered_at)`. Called by the engine per tick with whatever
    /// a detector returned; an empty slice is a no-op.
    async fn insert_batch(&self, signals: &[Signal]) -> RepositoryResult<()>;

    /// The latest severity emitted per pool by `detector` since `since` —
    /// the current suppression state for the engine's cooldown / escalation
    /// dedup. Pools with no signal from this detector in the window are
    /// absent from the map. Read under the SELECT already granted to the
    /// yog_signals role.
    async fn latest_severity_by_pool(
        &self,
        detector: &str,
        since: DateTime<Utc>,
    ) -> RepositoryResult<HashMap<Pubkey, Severity>>;
}

/// Read contract for the signal feed (api process, RO role).
///
/// Kept separate from [`SignalRepository`] — the engine's contract —
/// even though both are implemented by the same `Pg` struct: the two
/// consumers share no method, so each depends only on what it uses and
/// the api mock doesn't have to carry the engine's write/dedup methods
/// (same reasoning as `PoolAccountResolver` vs `PoolRepository`).
#[async_trait]
pub trait SignalFeedRepository: Send + Sync {
    /// Paginate the signal feed, ordered by `triggered_at DESC`,
    /// `id DESC` as tiebreaker (newest first).
    ///
    /// `severity`, when set, restricts the feed to that exact severity.
    /// `cursor` is `None` for the first page; for subsequent pages, pass
    /// the cursor returned by the previous call. `limit` is the maximum
    /// number of items to return; implementations may cap it to an upper
    /// bound.
    async fn list(
        &self,
        severity: Option<Severity>,
        cursor: Option<SignalCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<SignalRecord>>;
}
