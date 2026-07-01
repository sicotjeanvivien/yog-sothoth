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

use crate::{
    RepositoryResult,
    domain::{Severity, Signal},
};

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
