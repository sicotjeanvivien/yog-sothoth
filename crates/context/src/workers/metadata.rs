//! Metadata worker — discovers new mints and enriches them.
//!
//! STUB (commit 1): the struct and its `run` signature are in place
//! so the daemon can spawn it and the crate compiles. The real loop
//! — poll `pools` for missing mints, fetch Helius DAS, upsert — lands
//! in commit 2.

use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::error::WorkerError;

/// Worker that keeps `token_metadata` in sync with the mints seen in
/// `pools`.
pub struct MetadataWorker {}

impl MetadataWorker {
    /// Build the worker over the shared daemon state.
    pub fn new() -> Self {
        Self {}
    }

    /// Run the worker until the shutdown token is triggered.
    ///
    /// STUB: currently just waits for shutdown. Commit 2 fills in the
    /// poll/fetch/upsert loop.
    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        info!("MetadataWorker started (stub — no-op until commit 2)");
        shutdown.cancelled().await;
        info!("shutdown requested — metadata worker stopping");
        Ok(())
    }
}
