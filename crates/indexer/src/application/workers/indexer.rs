//! Indexer worker — consumes qualified signatures from the dispatcher and
//! drives `TransactionProcessor::index_transaction` with bounded concurrency.
//!
//! Responsibility split:
//! - `run` owns the receive loop and the shutdown semantics.
//! - `dispatch_one` handles a single qualified signature (permit + spawn).
//! - `index_one` runs inside the spawned task and owns per-signature logging.
//!
//! Error semantics:
//! - Per-signature failures are logged and counted, never propagated.
//!   A single failing transaction must not stop the pipeline.
//! - Loop-level failures (closed semaphore, closed channel in an
//!   unexpected state) are propagated as `IndexerWorkerError` and bubble
//!   up to `Daemon::run`.

use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    application::services::TransactionProcessor, error::IndexerWorkerError,
    infra::QualifiedSignature, utils::redact_api_key,
};

/// Maximum number of `index_transaction` calls running concurrently.
///
/// Sized against the Helius free tier (10 req/s) with headroom.
const MAX_CONCURRENT_INDEX_TASKS: usize = 15;

/// Worker that consumes qualified signatures and indexes them with
/// bounded concurrency.
pub(crate) struct IndexerWorker {
    processor: Arc<TransactionProcessor>,
    semaphore: Arc<Semaphore>,
}

impl IndexerWorker {
    pub(crate) fn new(processor: Arc<TransactionProcessor>) -> Self {
        Self {
            processor,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_INDEX_TASKS)),
        }
    }

    /// Drive the receive loop until the upstream channel closes or
    /// the shutdown token is triggered.
    pub(crate) async fn run(
        self,
        mut rx: mpsc::Receiver<QualifiedSignature>,
        shutdown: CancellationToken,
    ) -> Result<(), IndexerWorkerError> {
        info!("IndexerWorker started");

        loop {
            tokio::select! {
                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(qs) => self.dispatch_one(qs, rx.len()).await?,
                        None => {
                            info!("upstream channel closed — indexer worker stopping");
                            return Ok(());
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — indexer worker stopping");
                    return Ok(());
                }
            }
        }
    }

    /// Acquire a concurrency permit and spawn an indexing task.
    ///
    /// Blocks only on permit acquisition — indexing itself runs in a
    /// detached task so the receive loop can keep draining the channel.
    async fn dispatch_one(
        &self,
        qs: QualifiedSignature,
        queue_depth: usize,
    ) -> Result<(), IndexerWorkerError> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| IndexerWorkerError::SemaphoreClosed)?;

        debug!(
            queue_depth,
            permits_available = self.semaphore.available_permits(),
            protocol = %qs.protocol.as_str(),
            signature = %qs.signature,
            "dispatching signature to indexer service"
        );

        let processor = Arc::clone(&self.processor);
        tokio::spawn(async move {
            index_one(processor, qs).await;
            drop(permit);
        });

        Ok(())
    }
}

/// Index a single signature. Per-signature errors are logged and counted,
/// never propagated — they must not stop the pipeline.
async fn index_one(processor: Arc<TransactionProcessor>, qs: QualifiedSignature) {
    let QualifiedSignature {
        protocol,
        signature,
    } = qs;

    match processor.process(protocol, signature).await {
        Ok(()) => {
            debug!(%signature, "process ok");
        }
        Err(e) => {
            let msg = redact_api_key(&e.to_string());
            error!(error = %msg, %signature, "process failed");
        }
    }
}
