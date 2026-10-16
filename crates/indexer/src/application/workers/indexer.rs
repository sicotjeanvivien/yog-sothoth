//! Indexer worker: consumes what a [`TransactionSource`] delivered and drives
//! `TransactionProcessor::process_transaction` with bounded concurrency. Per-transaction
//! failures are logged and counted; only loop-level ones reach `Daemon::run`.
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

mod metrics;

pub(crate) use metrics::IndexerWorkerMetrics;

/// Worker that consumes delivered transactions and indexes them with bounded concurrency.
pub(crate) struct IndexerWorker {
    processor: Arc<TransactionProcessor>,
    semaphore: Arc<Semaphore>,
    /// The semaphore's full count: the stop asks for *all* permits back, which
    /// `available_permits()` cannot tell once tasks are in flight.
    max_concurrent: u32,
}

impl IndexerWorker {
    /// `max_concurrent` is bounded by the database pool minus its other users in this process,
    /// which only the composition root knows (`bootstrap::daemon::tasks::index_concurrency`).
    pub(crate) fn new(processor: Arc<TransactionProcessor>, max_concurrent: usize) -> Self {
        Self {
            processor,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent: u32::try_from(max_concurrent)
                .expect("the bound is derived from a connection pool, which is far below u32::MAX"),
        }
    }

    /// Drive the receive loop until the upstream channel closes or the shutdown token fires.
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
                // `biased`: a `dispatch_one` that returned because the token fired is not followed
                // by another message taken instead of the stop.
                biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — indexer worker stopping");
                    // What is still queued dies with the receiver: counted, since a source paid
                    // for it and nothing re-requests it.
                    let dropped = drain_and_count(&mut rx);
                    if dropped > 0 {
                        info!(dropped, "queued transactions dropped at shutdown");
                    }
                    await_in_flight(&self.semaphore, self.max_concurrent).await;
                    return Ok(());
                }

                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(ingested) => {
                            self.dispatch_one(ingested, rx.len(), &shutdown).await?
                        }
                        None => {
                            info!("upstream channel closed — indexer worker stopping");
                            await_in_flight(&self.semaphore, self.max_concurrent).await;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Acquire a permit and spawn an indexing task, detached, so the loop keeps draining.
    /// ⚠️ The wait is interruptible: `run`'s cancellation arm is not polled while this is
    /// pending, and nothing bounds how long a statement holds a permit.
    async fn dispatch_one(
        &self,
        ingested: IngestedTransaction,
        queue_depth: usize,
        shutdown: &CancellationToken,
    ) -> Result<(), IndexerWorkerError> {
        let permit = tokio::select! {
            biased;

            _ = shutdown.cancelled() => {
                IndexerWorkerMetrics::record_dropped(&ingested.protocol, "shutdown");
                return Ok(());
            }

            permit = Arc::clone(&self.semaphore).acquire_owned() => {
                permit.map_err(|_| IndexerWorkerError::SemaphoreClosed)?
            }
        };

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

/// Return once every detached indexing task has finished; unbounded, the grace is `Daemon::run`'s.
/// ⚠️ Without it a stop destroys an `INSERT`: the tasks' handles are dropped, so only their
/// permits coming back say they are done.
async fn await_in_flight(semaphore: &Semaphore, max_concurrent: u32) {
    let in_flight = max_concurrent as usize - semaphore.available_permits();
    if in_flight > 0 {
        info!(in_flight, "waiting for in-flight indexing to finish");
    }
    // Cannot fail unless the semaphore is closed, and nothing closes it.
    let _ = semaphore.acquire_many(max_concurrent).await;
}

/// Count, and drop, what is still in the channel when the worker stops; returned so a test can
/// assert it. ⚠️ A snapshot: a source may still be sending.
fn drain_and_count(rx: &mut mpsc::Receiver<IngestedTransaction>) -> usize {
    let mut dropped = 0usize;
    while let Ok(ingested) = rx.try_recv() {
        IndexerWorkerMetrics::record_dropped(&ingested.protocol, "shutdown_queued");
        dropped += 1;
    }
    dropped
}

#[cfg(test)]
#[path = "indexer/indexer_tests.rs"]
mod tests;

/// Index a single transaction; errors are logged and counted, never propagated.
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
