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
///
/// # ⚠️ What splitting the stages changed, and it is not nothing
///
/// A permit used to cover the fetch **and** the persist, because one worker did
/// both. The sustained request rate was therefore `15 / (fetch + persist)`, and
/// a slow database throttled the RPC as a side effect. A permit now covers the
/// fetch alone: the transaction goes into a 1 000-deep channel and the slot
/// frees at once, so the rate is `15 / fetch`.
///
/// In normal operation that is a few percent — a persist is short next to a
/// round-trip — and it is the *correct* shape: two resources, two bounds, each
/// beside the thing it protects. But when the database degrades the two
/// diverge, and this stage will keep asking at full rate while the queue fills,
/// which is when a rate-limited provider is least forgiving. The channel's
/// depth is what delays back-pressure reaching here; it is sized for memory
/// (see `INGESTED_CHANNEL_CAPACITY`), not for this. Nothing has measured which
/// of the two should give way, and until something has, this note is the whole
/// of what is known.
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
                // `biased`: the cancellation arm is checked first, so a
                // `dispatch_one` that returned *because* the token fired cannot
                // be followed by another message being taken instead of the
                // stop. Without it the loop would race a ready `recv` against a
                // ready token on every iteration.
                biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — fetch worker stopping");
                    return Ok(());
                }

                // ⚠️ **The consumer going away stops this stage**, which is the
                // port's second exit clause and was a promise nothing kept: the
                // send failure happens inside a detached task, so `run` never
                // learned of it and kept pulling signatures and paying for
                // `getTransaction` calls whose results had nowhere to go. Short
                // today, because the daemon cancels the token as soon as any
                // task returns — but "another task will stop us shortly" is not
                // what the contract says, and the gRPC source will be written
                // against the contract.
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

    /// Acquire a permit and spawn one fetch.
    ///
    /// Blocks only on permit acquisition — the fetch itself runs detached so
    /// the receive loop keeps draining the dispatcher.
    ///
    /// ⚠️ **The wait is interruptible**, which the port requires of every
    /// implementation: `run`'s cancellation arm is not polled while this future
    /// is pending, so waiting here on a bare `acquire_owned` would make a
    /// saturated stage unstoppable for as long as the permits stay held.
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
                // The signature is discarded, and that is a loss: it named a
                // transaction nothing will ask for again. Counted for the same
                // reason as the two in `fetch_one` — this stage's rule is
                // *counted* and stepped over.
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

/// Fetch one transaction and hand it downstream.
///
/// Every failure here is per-signature: logged, counted, stepped over.
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

    // Waiting on a full consumer is correct — it is the database being the
    // bottleneck, and dropping here would lose a transaction nothing would
    // re-request.
    //
    // ⚠️ **But the wait is interruptible**, and not by accident. It does
    // resolve on its own today, because `IndexerWorker` drops its receiver when
    // the token fires and the send then fails at once — which makes shutdown
    // depend on what the *consumer* happens to do. The port asks each source to
    // honour the token itself, so this one does.
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
