//! Indexer worker — consumes what a [`TransactionSource`] delivered and drives
//! `TransactionProcessor::process_transaction` with bounded concurrency.
//!
//! Responsibility split:
//! - `run` owns the receive loop and the shutdown semantics.
//! - `dispatch_one` handles a single transaction (permit + spawn).
//! - `index_one` runs inside the spawned task and owns per-transaction logging.
//!
//! Error semantics:
//! - Per-transaction failures are logged and counted, never propagated.
//!   A single failing transaction must not stop the pipeline.
//! - Loop-level failures (closed semaphore, closed channel in an
//!   unexpected state) are propagated as `IndexerWorkerError` and bubble
//!   up to `Daemon::run`.
//!
//! [`TransactionSource`]: crate::application::source::TransactionSource

use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    application::{services::TransactionProcessor, source::IngestedTransaction},
    error::IndexerWorkerError,
};

/// Worker that consumes delivered transactions and indexes them with
/// bounded concurrency.
pub(crate) struct IndexerWorker {
    processor: Arc<TransactionProcessor>,
    semaphore: Arc<Semaphore>,
}

impl IndexerWorker {
    /// # The concurrency bound is an argument, not a constant
    ///
    /// ⚠️ It used to be a `15` written here, sized against the RPC quota — and
    /// that number belonged to the fetch, which is now the business of the one
    /// source that has to fetch. What bounds *this* stage is the database: every
    /// task in flight holds a connection while it persists, so beyond what the
    /// pool holds an extra task adds no throughput — it queues and then fails
    /// on `acquire_timeout`.
    ///
    /// It is passed in rather than computed here because the bound is not the
    /// pool's size but the pool's size *minus its other users in this process*,
    /// and only the composition root knows who those are — see
    /// `bootstrap::daemon::index_concurrency`.
    pub(crate) fn new(processor: Arc<TransactionProcessor>, max_concurrent: usize) -> Self {
        Self {
            processor,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Drive the receive loop until the upstream channel closes or
    /// the shutdown token is triggered.
    pub(crate) async fn run(
        self,
        mut rx: mpsc::Receiver<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), IndexerWorkerError> {
        info!(
            max_concurrent = self.semaphore.available_permits(),
            "IndexerWorker started"
        );

        loop {
            tokio::select! {
                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(ingested) => self.dispatch_one(ingested, rx.len()).await?,
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
        ingested: IngestedTransaction,
        queue_depth: usize,
    ) -> Result<(), IndexerWorkerError> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| IndexerWorkerError::SemaphoreClosed)?;

        debug!(
            queue_depth,
            permits_available = self.semaphore.available_permits(),
            protocol = %ingested.protocol.as_str(),
            signature = %ingested.transaction.position.signature,
            "dispatching transaction to indexer service"
        );

        let processor = Arc::clone(&self.processor);
        tokio::spawn(async move {
            index_one(processor, ingested).await;
            drop(permit);
        });

        Ok(())
    }
}

/// Index a single transaction. Per-transaction errors are logged and counted,
/// never propagated — they must not stop the pipeline.
async fn index_one(processor: Arc<TransactionProcessor>, ingested: IngestedTransaction) {
    let IngestedTransaction {
        protocol,
        transaction,
    } = ingested;
    let signature = transaction.position.signature;

    match processor.process_transaction(protocol, &transaction).await {
        Ok(()) => {
            debug!(%signature, "process ok");
        }
        Err(e) => {
            let msg = e.to_string();
            error!(error = %msg, %signature, "process failed");
        }
    }
}
