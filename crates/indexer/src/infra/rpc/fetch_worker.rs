//! The JSON-RPC path's third stage: ask for what the dispatcher named.
//!
//! This stage has **no counterpart on the gRPC path** and that is the whole
//! reason it lives here rather than in `application`. `logsSubscribe` pushes a
//! signature and some text; the transaction has to be fetched back over HTTP,
//! one call each. Yellowstone delivers the transaction itself, so a source
//! built on it has nothing to fetch and no quota to respect.
//!
//! What it produces is the neutral [`IngestedTransaction`] — from here on the
//! two paths are the same pipeline.
//!
//! # Error semantics
//!
//! Per-signature failures are counted and stepped over: a transaction that the
//! RPC will not return, or that will not translate, must not stop the fleet.
//! Only the loop-level failure — a closed semaphore — is propagated.

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
///
/// ⚠️ **Sized against the RPC quota, and nothing else** — the Helius free tier
/// at 10 req/s, with headroom. It travelled here from `IndexerWorker`, where it
/// had come to look like a general concurrency setting; it never was one. The
/// consumer downstream is bounded by the database connection pool instead,
/// which is a different resource and therefore a different number.
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
                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(qs) => self.dispatch_one(qs, &downstream, rx.len()).await?,
                        None => {
                            info!("upstream channel closed — fetch worker stopping");
                            return Ok(());
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — fetch worker stopping");
                    return Ok(());
                }
            }
        }
    }

    /// Acquire a permit and spawn one fetch.
    ///
    /// Blocks only on permit acquisition — the fetch itself runs detached so
    /// the receive loop keeps draining the dispatcher.
    async fn dispatch_one(
        &self,
        qs: QualifiedSignature,
        downstream: &mpsc::Sender<IngestedTransaction>,
        queue_depth: usize,
    ) -> Result<(), SourceError> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| SourceError::SemaphoreClosed { stage: "fetch" })?;

        debug!(
            queue_depth,
            permits_available = self.semaphore.available_permits(),
            protocol = %qs.protocol.as_str(),
            signature = %qs.signature,
            "dispatching signature to the fetcher"
        );

        let fetcher = Arc::clone(&self.fetcher);
        let downstream = downstream.clone();
        tokio::spawn(async move {
            fetch_one(fetcher, qs, downstream).await;
            drop(permit);
        });

        Ok(())
    }
}

/// Fetch one transaction and hand it downstream.
///
/// Every failure here is per-signature: logged, counted, stepped over.
async fn fetch_one(
    fetcher: Arc<TransactionFetcher>,
    qs: QualifiedSignature,
    downstream: mpsc::Sender<IngestedTransaction>,
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

    // The RPC response becomes the neutral transaction here, on the side that
    // knows what a `getTransaction` response looks like. A malformation — no
    // signature, no `blockTime` — is a transaction-level failure and takes the
    // same exit as a fetch failure: one log, one count, nothing sent on.
    //
    // ⚠️ It is counted under the fetch family with `reason="adapt"` rather than
    // under a name of its own. What the family answers is "this signature
    // produced nothing usable", which is the question an operator alerts on,
    // and the reason label is what separates a rate limit from a malformed
    // response. Before the fetch moved, this same failure was one of the
    // `index_transaction_exited{outcome="extract_failure"}` — it has to be
    // counted somewhere, and it can no longer be counted there.
    let transaction = match from_rpc(&fetched) {
        Ok(tx) => tx,
        Err(e) => {
            FetchMetrics::record_failure(&protocol, "adapt");
            error!(%signature, error = %e, "adapting the RPC response failed");
            return;
        }
    };

    // ⚠️ A closed channel is the consumer being gone, which is a shutdown in
    // progress and not this signature's problem. It is logged once here and
    // read by the source, whose `downstream` sender closing is what ends the
    // run — see `RpcTransactionSource::run`.
    if downstream
        .send(IngestedTransaction {
            protocol,
            transaction,
        })
        .await
        .is_err()
    {
        debug!(%signature, "downstream closed — dropping fetched transaction");
    }
}
