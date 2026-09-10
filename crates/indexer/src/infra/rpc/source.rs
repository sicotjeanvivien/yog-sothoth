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
//! # ⚠️ Untested, and for the same reason as its sibling
//!
//! Nothing here can be exercised without a Solana endpoint. What *is* testable
//! was already elsewhere before this module existed — the filter chain in
//! `dispatcher`, the adapter in `transaction_adapter` with its 92 fixtures —
//! and this file adds no logic of its own beyond wiring and the reading of how
//! a stage ending should end the whole source.

use std::sync::Arc;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_core::domain::Protocol;

use crate::{
    application::source::{IngestedTransaction, TransactionSource},
    error::SourceError,
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
    async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.listener.watch_pool(protocol, pool_address).await;
    }

    /// Run the three stages until the first of them returns.
    ///
    /// ⚠️ **Any stage ending ends the source**, including a clean `Ok(())`.
    /// That is not pessimism: these three are one pipeline, so a dispatcher
    /// that stopped leaves a listener filling a channel nobody drains, and a
    /// fetch stage that stopped leaves a source holding subscriptions and
    /// delivering nothing. Returning is what lets `Daemon::run` cancel the
    /// shared token and stop the process, rather than leave it running and
    /// silent — the failure mode that is hardest to notice.
    async fn run(
        &self,
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), SourceError> {
        let (raw_tx, raw_rx) = mpsc::channel::<RawLogEvent>(STAGE_CHANNEL_CAPACITY);
        let (sig_tx, sig_rx) = mpsc::channel::<QualifiedSignature>(STAGE_CHANNEL_CAPACITY);

        let listener_task = spawn_listener(Arc::clone(&self.listener), raw_tx, shutdown.clone());
        let dispatcher_task = spawn_dispatcher(
            Arc::clone(&self.dispatcher),
            raw_rx,
            sig_tx,
            shutdown.clone(),
        );
        let fetch_task = spawn_fetch(
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
        tokio::select! {
            biased;

            result = listener_task => join("listener", result)?,
            result = dispatcher_task => join("dispatcher", result)?,
            result = fetch_task => join("fetch worker", result)?,
        }

        Ok(())
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

/// Normalise a joined stage: its own error, or the panic that ate it.
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
        Err(e) => Err(SourceError::TaskPanicked {
            task,
            reason: e.to_string(),
        }),
    }
}
