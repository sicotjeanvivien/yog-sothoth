//! Price worker — periodically prices every known mint.
//!
//! STUB (commit 1): the struct and its `run` signature are in place
//! so the daemon can spawn it and the crate compiles. The real
//! interval loop — fetch Jupiter for all known mints, insert a price
//! batch — lands in commit 3.

use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::error::WorkerError;

/// Worker that records a USD price for every known mint on a fixed
/// interval.
pub struct PriceWorker {}

impl PriceWorker {
    /// Build the worker over the shared daemon state.
    pub fn new() -> Self {
        Self {}
    }

    /// Run the worker until the shutdown token is triggered.
    ///
    /// STUB: currently just waits for shutdown. Commit 3 fills in the
    /// interval-driven fetch/insert loop.
    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        info!("PriceWorker started (stub — no-op until commit 3)");
        shutdown.cancelled().await;
        info!("shutdown requested — price worker stopping");
        Ok(())
    }
}
