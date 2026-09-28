//! Slots for the expensive database work, shared by everything that does it.
//!
//! The API's pool has 10 connections. The slow routes (1.8–3.7 s each, measured
//! 25 September 2026) and the shared-result computations behind
//! `/api/pools/top` and `/api/stats` all take one slot here, so together they
//! can never hold more than the slots — the light routes and the signal
//! poller keep the rest of the pool.
//!
//! A slot is taken by whoever **does** the work: a route while it runs, a
//! cached computation while it computes — never a caller waiting for a cached
//! value. Waiters holding slots is what turned 30 `/top` requests on a cold
//! cache into 24 × `503` for a single computation.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use yog_core::RepositoryError;

#[derive(Clone)]
pub(crate) struct WorkSlots {
    slots: Arc<Semaphore>,
    wait: Duration,
}

impl WorkSlots {
    pub(crate) fn new(permits: usize, wait: Duration) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(permits)),
            wait,
        }
    }

    /// A slot, if one frees up within the wait. Held until dropped.
    pub(crate) async fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        tokio::time::timeout(self.wait, self.slots.clone().acquire_owned())
            .await
            .ok()?
            .ok()
    }
}

/// What a service returns when no slot freed up for its computation: a time
/// budget exceeded, which the HTTP layer answers `503` with `Retry-After`.
pub(crate) fn no_work_slot() -> RepositoryError {
    RepositoryError::Timeout("no work slot freed up within the wait".to_string())
}
