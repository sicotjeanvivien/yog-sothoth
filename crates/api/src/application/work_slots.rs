//! Slots for the expensive database work: the slow routes and the cached computations
//! together never hold more than the slots, so the light routes and the poller keep the rest
//! of the pool. ⚠️ Taken by whoever does the work, never by a caller waiting for a cached value.

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
