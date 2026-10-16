//! The JSON-RPC acquisition model, behind the port: three stages joined by two bounded channels.
//!
//! ```text
//! RpcListener ──RawLogEvent──▶ SignatureDispatcher ──QualifiedSignature──▶ FetchWorker
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use yog_bootstrap::TaskEnd;
use yog_core::domain::Protocol;

use crate::{
    application::source::{IngestedTransaction, TransactionSource},
    error::SourceError,
    infra::rpc::{
        FetchWorker, QualifiedSignature, RawLogEvent, RpcListener, SignatureDispatcher,
        TransactionFetcher,
    },
};

/// How many `RawLogEvent`s and `QualifiedSignature`s may queue between stages; both are small,
/// so a deep queue is cheap.
const STAGE_CHANNEL_CAPACITY: usize = 10_000;

/// The names the three stages answer to, in the logs and in the wait after the `select!`.
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

    /// Run the three stages until the first returns, then cancel the token and wait for the others.
    /// ⚠️ Any stage ending ends the source, `Ok(())` included: they are one pipeline, and a process
    /// left running and silent is the hardest failure to notice.
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

        // ⚠️ `biased`, in pipeline order: a listener failing cascades into the other two ending
        // `Ok(())`, and an unbiased pick could report a dead ingestion as a clean stop.
        let (ended, first) = tokio::select! {
            biased;

            result = &mut listener_task => (LISTENER, join(LISTENER, result)),
            result = &mut dispatcher_task => (DISPATCHER, join(DISPATCHER, result)),
            result = &mut fetch_task => (FETCH, join(FETCH, result)),
        };

        // ⚠️ Cancelled here: the cascade only runs downstream, so a fetch stage that stopped by
        // itself would leave the wait below on a fleet nobody told to stop.
        shutdown.cancel();

        // Unbounded: the grace belongs to `Daemon::run`. ⚠️ What the others report counts: a
        // stage can fail while the pipeline winds down, after another returned `Ok(())`.
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
    // The dispatcher cannot fail once built: there is no error to swallow.
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
/// Wait for the stages that have not ended yet; the first failure wins, `first` being what the
/// stage named by `ended` reported.
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

/// Normalise a joined stage: its own error, the panic that ate it, or the shutdown that destroyed it.
/// ⚠️ Nothing aborts these stages, so a cancellation is the runtime torn down around one, not a
/// crash: `TaskPanicked` would name one that never happened.
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
#[path = "tests/source_tests.rs"]
mod tests;
