//! The JSON-RPC acquisition model, behind the port.
//!
//! Three stages, connected by two bounded channels that never leave this
//! module:
//!
//! ```text
//! RpcListener ──RawLogEvent──▶ SignatureDispatcher ──QualifiedSignature──▶ FetchWorker
//!  (fleet of                    (failed / invocation                       (bounded by
//!   WebSockets)                  filters)                                   the RPC quota)
//! ```
//!
//! and then one `IngestedTransaction` per surviving signature, out through the
//! port. That the model takes three stages, two channels and a fleet of
//! WebSockets is precisely what the port hides: the gRPC source has one
//! connection and no fetch, and the daemon has to know neither.
//!
//! # ⚠️ Almost untested, and for the same reason as its sibling
//!
//! `run` cannot be exercised without a Solana endpoint, and most of what *is*
//! testable was already elsewhere before this module existed — the filter chain
//! in `dispatcher`, the adapter in `transaction_adapter` with its 92 fixtures.
//!
//! One piece is neither: `drain_stages` decides what the source *reports* once
//! a stage has ended, and that is logic, not wiring. It was written as a loop
//! inside `run` first, where nothing could reach it — and the same rule written
//! the same way one level up had already let a dead ingestion exit 0. It takes
//! its handles as an argument so a test can hand it three that ended on
//! purpose.

use std::sync::Arc;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use yog_core::domain::Protocol;

use crate::{
    application::source::{IngestedTransaction, TransactionSource},
    error::{SourceError, TaskEnd},
    infra::rpc::{
        FetchWorker, QualifiedSignature, RawLogEvent, RpcListener, SignatureDispatcher,
        TransactionFetcher,
    },
};

/// How many `RawLogEvent`s and `QualifiedSignature`s may queue between stages.
///
/// Both carry a signature and a little text, so a deep queue is cheap here —
/// unlike the port's channel, which carries whole transactions. Unchanged from
/// when these two channels were wired by the daemon.
const STAGE_CHANNEL_CAPACITY: usize = 10_000;

/// The names the three stages answer to — in the logs, and in the wait that
/// follows the `select!`. Named once because each is now written at two sites.
const LISTENER: &str = "listener";
const DISPATCHER: &str = "dispatcher";
const FETCH: &str = "fetch worker";

/// The notify-then-ask source: a fleet of `logsSubscribe` subscriptions, a
/// filter chain, and one `getTransaction` per surviving signature.
pub(crate) struct RpcTransactionSource {
    listener: Arc<RpcListener>,
    dispatcher: Arc<SignatureDispatcher>,
    fetcher: Arc<TransactionFetcher>,
}

impl RpcTransactionSource {
    pub(crate) fn new(
        listener: Arc<RpcListener>,
        dispatcher: Arc<SignatureDispatcher>,
        fetcher: Arc<TransactionFetcher>,
    ) -> Self {
        Self {
            listener,
            dispatcher,
            fetcher,
        }
    }
}

#[async_trait]
impl TransactionSource for RpcTransactionSource {
    async fn watch_protocol(&self, protocol: Protocol) {
        self.listener.watch(protocol).await;
    }

    async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.listener.watch_pool(protocol, pool_address).await;
    }

    /// Run the three stages until the first of them returns — then cancel the
    /// token and wait for the other two.
    ///
    /// ⚠️ **Any stage ending ends the source**, including a clean `Ok(())`.
    /// That is not pessimism: these three are one pipeline, so a dispatcher
    /// that stopped leaves a listener filling a channel nobody drains, and a
    /// fetch stage that stopped leaves a source holding subscriptions and
    /// delivering nothing. Returning is what stops the process, rather than
    /// leave it running and silent — the failure mode that is hardest to
    /// notice.
    ///
    /// ⚠️ **But returning on the first one alone was a lie, and it was measured
    /// on 14 September 2026**: on a Ctrl-C the dispatcher stops in microseconds
    /// while the fleet is still unsubscribing, so `biased` or not, this
    /// returned "dispatcher stopped" and the daemon wrote "transaction source
    /// stopped" 7 ms after the signal — with two WebSocket sessions still open
    /// and about to be destroyed with the runtime. The outcome reported is
    /// still the first stage's, because it is still the cause; what changed is
    /// that the source no longer claims to be done before it is.
    async fn run(
        &self,
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), SourceError> {
        let (raw_tx, raw_rx) = mpsc::channel::<RawLogEvent>(STAGE_CHANNEL_CAPACITY);
        let (sig_tx, sig_rx) = mpsc::channel::<QualifiedSignature>(STAGE_CHANNEL_CAPACITY);

        let mut listener_task =
            spawn_listener(Arc::clone(&self.listener), raw_tx, shutdown.clone());
        let mut dispatcher_task = spawn_dispatcher(
            Arc::clone(&self.dispatcher),
            raw_rx,
            sig_tx,
            shutdown.clone(),
        );
        let mut fetch_task = spawn_fetch(
            Arc::clone(&self.fetcher),
            sig_rx,
            downstream,
            shutdown.clone(),
        );

        info!("RPC transaction source started");

        // ⚠️ `biased`, and the order is the pipeline's own. These three fail in
        // a cascade: the listener returning `Err(AllWorkersGaveUp)` drops
        // `raw_tx`, which makes the dispatcher exit `Ok(())`, which drops
        // `sig_tx`, which stops the fetch stage — so by the next poll two or
        // three handles are ready at once. An unbiased `select!` picks among
        // them at random, and reporting "dispatcher stopped, `Ok(())`" for a
        // provider that exhausted every retry budget is a success exit code for
        // a dead ingestion. Polling in pipeline order makes the *cause* win the
        // race against the consequences it just created.
        //
        // Which stage it was comes back with its outcome: its handle has been
        // polled to completion, and `tokio` panics on one polled again, so the
        // wait below has to step over it.
        let (ended, first) = tokio::select! {
            biased;

            result = &mut listener_task => (LISTENER, join(LISTENER, result)),
            result = &mut dispatcher_task => (DISPATCHER, join(DISPATCHER, result)),
            result = &mut fetch_task => (FETCH, join(FETCH, result)),
        };

        // ⚠️ **Cancelled here, not left to `Daemon::run`.** One stage returning
        // ends the pipeline, and the cascade that used to carry that news only
        // runs downstream: a fetch stage that stopped on its own drops nothing
        // the listener is waiting for, so without this the wait below would sit
        // on a fleet nobody has told to stop.
        shutdown.cancel();

        // Then wait for the other two. A source that reports "stopped" while
        // its workers are still unsubscribing tells `Daemon::run` the stage is
        // done, and the daemon's grace — the only clock in the stop — starts
        // running out on stages it believes have already finished.
        //
        // Unbounded on purpose: the bound belongs to `Daemon::run`, which holds
        // one grace for the whole process. A second timeout here would be a
        // second answer to the same question.
        //
        // ⚠️ **And what they report counts.** A stage can fail *while* the
        // pipeline winds down — a panic in the unsubscribe path, say — long
        // after another stage returned `Ok(())`. Dropping those outcomes would
        // leave the failure with no error, no log line and no exit code, which
        // is the defect `Daemon::run`'s `Stop` exists to prevent one level up.
        // Same rule here, first failure wins: not shared with it because the
        // daemon's accumulator also carries a grace and a list of stages that
        // outlived it, and neither has any meaning at this level.
        drain_stages(
            ended,
            first,
            [
                (LISTENER, &mut listener_task),
                (DISPATCHER, &mut dispatcher_task),
                (FETCH, &mut fetch_task),
            ],
        )
        .await
    }
}

// ── Task spawners ────────────────────────────────────────────────────────────

fn spawn_listener(
    listener: Arc<RpcListener>,
    raw_tx: mpsc::Sender<RawLogEvent>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move {
        listener
            .run(raw_tx, shutdown)
            .await
            .map_err(SourceError::from)
    })
}

fn spawn_dispatcher(
    dispatcher: Arc<SignatureDispatcher>,
    raw_rx: mpsc::Receiver<RawLogEvent>,
    sig_tx: mpsc::Sender<QualifiedSignature>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    // The dispatcher cannot fail once built — see its `run`. `Ok(())` here is
    // not a swallowed error: there is none to swallow.
    tokio::spawn(async move {
        dispatcher.run(raw_rx, sig_tx, shutdown).await;
        Ok(())
    })
}

fn spawn_fetch(
    fetcher: Arc<TransactionFetcher>,
    sig_rx: mpsc::Receiver<QualifiedSignature>,
    downstream: mpsc::Sender<IngestedTransaction>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    let worker = FetchWorker::new(fetcher);
    tokio::spawn(async move { worker.run(sig_rx, downstream, shutdown).await })
}

/// Wait for the stages that have not ended yet, and keep the first failure.
///
/// `first` is what the stage named by `ended` reported; the others are joined
/// in order and their outcome adopted only if nothing has failed yet.
async fn drain_stages(
    ended: &str,
    first: Result<(), SourceError>,
    stages: [(&'static str, &mut JoinHandle<Result<(), SourceError>>); 3],
) -> Result<(), SourceError> {
    let mut outcome = first;
    for (name, handle) in stages {
        if ended != name {
            let reported = join(name, handle.await);
            if outcome.is_ok() {
                outcome = reported;
            }
        }
    }
    outcome
}

/// Normalise a joined stage: its own error, the panic that ate it, or the
/// shutdown that destroyed it before it could answer.
///
/// ⚠️ **The last two are one type in `tokio` and must not be one here** — see
/// [`TaskEnd`]. Nothing aborts these three, so a cancellation means the runtime
/// was torn down around the stage: the work was cut short, it did not fail, and
/// `TaskPanicked` would name a crash that never happened.
fn join(
    task: &'static str,
    result: Result<Result<(), SourceError>, tokio::task::JoinError>,
) -> Result<(), SourceError> {
    match result {
        Ok(Ok(())) => {
            info!("{task} stopped");
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(e) => match TaskEnd::from(&e) {
            TaskEnd::Panicked => Err(SourceError::TaskPanicked {
                task,
                reason: e.to_string(),
            }),
            TaskEnd::Cancelled => {
                debug!(task, "stage destroyed before it could finish stopping");
                Ok(())
            }
        },
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
