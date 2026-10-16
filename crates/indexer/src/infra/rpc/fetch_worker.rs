//! The JSON-RPC path's third stage, with no gRPC counterpart: fetch back over HTTP what the
//! dispatcher named. Per-signature failures are counted and stepped over; only a closed
//! semaphore is propagated.

use std::{sync::Arc, time::Instant};

use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use super::{FetchError, QualifiedSignature, TransactionFetcher, from_rpc};
use crate::{
    application::source::IngestedTransaction, error::SourceError,
    infra::rpc::fetch_metrics::FetchMetrics,
};

/// Maximum number of `getTransaction` calls in flight.
/// ⚠️ Sized against the RPC quota alone; a permit covers the fetch, not the persist, so a slow
/// database does not slow the fetch: it fills the channel instead.
const MAX_CONCURRENT_FETCHES: usize = 15;

/// Fetches transactions for qualified signatures, with bounded concurrency.
pub(crate) struct FetchWorker {
    fetcher: Arc<TransactionFetcher>,
    semaphore: Arc<Semaphore>,
}

impl FetchWorker {
    pub(crate) fn new(fetcher: Arc<TransactionFetcher>) -> Self {
        Self {
            fetcher,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES)),
        }
    }

    /// Drive the receive loop until the upstream channel closes or the shutdown
    /// token is triggered.
    pub(crate) async fn run(
        self,
        mut rx: mpsc::Receiver<QualifiedSignature>,
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), SourceError> {
        info!(
            max_concurrent = MAX_CONCURRENT_FETCHES,
            "FetchWorker started"
        );

        loop {
            tokio::select! {
                // `biased`: a `dispatch_one` that returned because the token fired is not followed
                // by another message taken instead of the stop.
                biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — fetch worker stopping");
                    return Ok(());
                }

                // ⚠️ The consumer going away stops this stage: the send fails in a detached task,
                // so without this arm `run` would keep paying for fetches with nowhere to go.
                _ = downstream.closed() => {
                    info!("downstream gone — fetch worker stopping");
                    return Ok(());
                }

                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(qs) => self.dispatch_one(qs, &downstream, rx.len(), &shutdown).await?,
                        None => {
                            info!("upstream channel closed — fetch worker stopping");
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Acquire a permit and spawn one fetch, detached, so the loop keeps draining the dispatcher.
    /// ⚠️ The wait is interruptible: `run`'s cancellation arm is not polled while this is pending.
    async fn dispatch_one(
        &self,
        qs: QualifiedSignature,
        downstream: &mpsc::Sender<IngestedTransaction>,
        queue_depth: usize,
        shutdown: &CancellationToken,
    ) -> Result<(), SourceError> {
        let permit = tokio::select! {
            biased;

            _ = shutdown.cancelled() => {
                // A loss, counted: the signature named a transaction nothing will ask for again.
                FetchMetrics::record_dropped(&qs.protocol, "shutdown_before_fetch");
                return Ok(());
            }

            permit = Arc::clone(&self.semaphore).acquire_owned() => {
                permit.map_err(|_| SourceError::SemaphoreClosed { stage: "fetch" })?
            }
        };

        debug!(
            queue_depth,
            permits_available = self.semaphore.available_permits(),
            protocol = %qs.protocol.as_str(),
            signature = %qs.signature,
            "dispatching signature to the fetcher"
        );

        let fetcher = Arc::clone(&self.fetcher);
        let downstream = downstream.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            fetch_one(fetcher, qs, downstream, shutdown).await;
            drop(permit);
        });

        Ok(())
    }
}

/// Fetch one transaction and hand it downstream; every failure is per-signature, counted.
async fn fetch_one(
    fetcher: Arc<TransactionFetcher>,
    qs: QualifiedSignature,
    downstream: mpsc::Sender<IngestedTransaction>,
    shutdown: CancellationToken,
) {
    let QualifiedSignature {
        protocol,
        signature,
    } = qs;

    let start = Instant::now();
    let result = fetcher.fetch(signature).await;
    FetchMetrics::record_duration(&protocol, start.elapsed().as_secs_f64());

    let fetched = match result {
        Ok(tx) => tx,
        Err(FetchError::NotFound) => {
            FetchMetrics::record_not_found(&protocol);
            debug!(%signature, "transaction not found by the RPC");
            return;
        }
        Err(e) => {
            FetchMetrics::record_failure(&protocol, e.metric_label());
            error!(%signature, error = %e, "fetch failed");
            return;
        }
    };

    // A malformation takes the fetch failure's exit, counted `reason="adapt"`: the family answers
    // "this signature produced nothing usable", what an operator alerts on.
    let transaction = match from_rpc(&fetched) {
        Ok(tx) => tx,
        Err(e) => {
            FetchMetrics::record_failure(&protocol, "adapt");
            error!(%signature, error = %e, "adapting the RPC response failed");
            return;
        }
    };

    // Waiting on a full consumer is the database being the bottleneck: dropping would lose a
    // transaction nothing re-requests. ⚠️ Interruptible: shutdown must not depend on the consumer.
    let ingested = IngestedTransaction {
        protocol,
        transaction,
    };
    tokio::select! {
        biased;

        _ = shutdown.cancelled() => {
            FetchMetrics::record_dropped(&protocol, "shutdown");
            debug!(%signature, "shutdown while handing over — dropping fetched transaction");
        }

        result = downstream.send(ingested) => {
            if result.is_err() {
                FetchMetrics::record_dropped(&protocol, "downstream_closed");
                debug!(%signature, "downstream closed — dropping fetched transaction");
            }
        }
    }
}
