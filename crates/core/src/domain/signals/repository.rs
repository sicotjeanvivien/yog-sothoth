//! Signal repository trait.
//!
//! Persistence contract for the `signals` hypertable. The engine calls
//! it after a detector tick to persist the returned signals; detectors
//! themselves never write. The concrete `Pg` implementation lives in
//! `yog-persistence`.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::Signal};

/// Write contract for emitted signals.
#[async_trait]
pub trait SignalRepository: Send + Sync {
    /// Persist a batch of freshly-detected signals in one round-trip.
    /// `signals` is append-only — each is a new row keyed by
    /// `(id, triggered_at)`. Called by the engine per tick with whatever
    /// a detector returned; an empty slice is a no-op.
    async fn insert_batch(&self, signals: &[Signal]) -> RepositoryResult<()>;
}
