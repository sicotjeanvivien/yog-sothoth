use std::{collections::HashSet, sync::Arc};

use solana_pubkey::Pubkey;
use tokio::{
    sync::{Mutex, broadcast, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use yog_bootstrap::{Endpoint, SecretUrl};
use yog_core::domain::Protocol;

use crate::{
    error::{RpcListenerError, SubscriptionWorkerError},
    infra::{
        Credential,
        rpc::{RawLogEvent, SubscriptionEvent, SubscriptionTarget, SubscriptionWorker},
        scheme,
    },
};

/// Default size of the broadcast channel carrying `SubscriptionEvent`s.
/// Oversized on purpose — with a handful of workers emitting occasional
/// events, we never want the listener to drop telemetry because it lagged
/// for a few milliseconds.
const EVENTS_CHANNEL_CAPACITY: usize = 256;

/// Orchestrator for a pool of `SubscriptionWorker`s.
///
/// Responsibilities kept deliberately minimal:
/// - build the list of `SubscriptionTarget`s from what is watched
/// - spawn one `SubscriptionWorker` per target
/// - consume their `SubscriptionEvent`s (log, metrics, tracking)
/// - escalate to the Daemon when *all* workers have given up
///
/// The listener does NOT:
/// - manage retries (each worker owns its retry budget)
/// - force a global reconnect when one worker dies (siblings keep running)
/// - respawn dead workers (future work — see roadmap)
pub(crate) struct RpcListener {
    /// The whole endpoint, not just its URL: a provider may carry its key in
    /// the query string **or** in a metadata header, and which one is the
    /// operator's business — see `infra::credential`. The fleet clones the
    /// assembled URL and the validated header once per worker.
    endpoint: Endpoint,
    /// Every address to subscribe to, with the protocol it belongs to — a
    /// program id or a pool. **One set, and no scope**: a `logsSubscribe`
    /// target is one `mentions` pubkey whichever it is, and what goes in here
    /// is decided upstream, by the daemon's registration.
    watched: Mutex<HashSet<(Protocol, Pubkey)>>,
    worker_max_retries: u32,
}

impl RpcListener {
    pub(crate) fn new(endpoint: Endpoint, worker_max_retries: u32) -> Self {
        Self {
            endpoint,
            watched: Mutex::new(HashSet::new()),
            worker_max_retries,
        }
    }

    /// Watch a whole protocol: its program id becomes a target.
    ///
    /// It carried a `_` prefix from the day it was written until 10 September
    /// 2026, because nothing called it and `INGEST_SCOPE=protocols` was refused
    /// at load time for exactly that reason. The gRPC slice gave it a caller:
    /// the source's `watch_protocol`, driven from the daemon at start-up.
    pub(crate) async fn watch(&self, protocol: Protocol) {
        self.watched
            .lock()
            .await
            .insert((protocol, protocol.program_id()));
    }

    pub(crate) async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.watched.lock().await.insert((protocol, pool_address));
    }

    /// Spawn workers, supervise them, and return when they're all done.
    ///
    /// Returns `Err(AllWorkersGaveUp)` with per-worker details when every
    /// spawned worker has exhausted its retry budget. Returns `Ok(())` when
    /// the shutdown token was cancelled before that happened.
    pub(crate) async fn run(
        self: Arc<Self>,
        dispatcher_tx: mpsc::Sender<RawLogEvent>,
        shutdown: CancellationToken,
    ) -> Result<(), RpcListenerError> {
        // Validated once, before a single worker is spawned: a malformed header
        // is a configuration failure, and letting each worker rediscover it
        // would spend a retry budget on something that cannot get better.
        let credential = Credential::new(self.endpoint.header())?;
        // Same reason, same place: a URL this path cannot speak would otherwise
        // cost a retry budget per worker — see `scheme::WEBSOCKET`.
        scheme::check(&self.endpoint.url(), &scheme::WEBSOCKET)
            .map_err(|reason| RpcListenerError::InvalidEndpoint { reason })?;
        info!(
            endpoint = %self.endpoint,
            header = credential.name().unwrap_or("none"),
            "RPC listener starting"
        );

        let targets = self.build_subscription_targets().await?;
        let total = targets.len();

        let (events_tx, _events_rx) =
            broadcast::channel::<SubscriptionEvent>(EVENTS_CHANNEL_CAPACITY);

        info!(count = total, "spawning subscription workers");

        let mut handles: Vec<WorkerHandle> = targets
            .into_iter()
            .map(|target| {
                spawn_worker(
                    self.endpoint.url(),
                    credential.clone(),
                    target,
                    self.worker_max_retries,
                    dispatcher_tx.clone(),
                    events_tx.clone(),
                    shutdown.clone(),
                )
            })
            .collect();

        // Subscribe before dropping the original sender — guarantees we don't
        // miss events emitted in the tiny window before the loop starts.
        let mut events_rx = events_tx.subscribe();
        // Drop the listener's own sender. Workers keep their clones alive
        // as long as they run; when the last one exits, the receiver closes
        // naturally. This is how we detect "all workers gone" without
        // needing a counter.
        drop(events_tx);

        let mut gave_up: Vec<WorkerFailure> = Vec::new();

        // Supervision loop.
        loop {
            tokio::select! {
                biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — awaiting workers");
                    break;
                }

                event = events_rx.recv() => {
                    match event {
                        Ok(ev) => handle_event(&ev),
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(missed = n, "listener lagged on events channel");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            debug!("all workers released their event senders");
                            break;
                        }
                    }
                }
            }
        }

        // Join all handles — drives them to completion and collects outcomes.
        for h in handles.drain(..) {
            match h.handle.await {
                Ok(Ok(())) => {
                    debug!(
                        protocol = %h.target.protocol.as_str(),
                        mention = %h.target.mention,
                        "worker exited cleanly"
                    );
                }
                Ok(Err(e)) => push_failure(&mut gave_up, &e),
                Err(e) => {
                    error!(
                        protocol = %h.target.protocol.as_str(),
                        mention = %h.target.mention,
                        error = %e,
                        "worker task panicked"
                    );
                    gave_up.push(WorkerFailure {
                        protocol: h.target.protocol,
                        mention: h.target.mention,
                        reason: format!("panic: {e}"),
                    });
                }
            }
        }

        if shutdown.is_cancelled() {
            info!("RPC listener stopped cleanly");
            return Ok(());
        }

        if gave_up.len() == total && total > 0 {
            return Err(RpcListenerError::AllWorkersGaveUp {
                failures: "gave_up".to_string(),
            });
        }

        Ok(())
    }

    async fn build_subscription_targets(
        &self,
    ) -> Result<Vec<SubscriptionTarget>, RpcListenerError> {
        let targets: Vec<_> = self
            .watched
            .lock()
            .await
            .iter()
            .map(|&(protocol, mention)| SubscriptionTarget::new(protocol, mention))
            .collect();

        if targets.is_empty() {
            return Err(RpcListenerError::NoSubscriptionTargets);
        }
        Ok(targets)
    }
}

// ---------------------------------------------------------------------------
// Supervision helpers
// ---------------------------------------------------------------------------

/// Per-worker failure detail — bubbled up in `AllWorkersGaveUp`.
#[derive(Debug, Clone)]
pub(crate) struct WorkerFailure {
    #[allow(dead_code)]
    pub protocol: Protocol,
    #[allow(dead_code)]
    pub mention: Pubkey,
    #[allow(dead_code)]
    pub reason: String,
}

/// Bundle that keeps a worker handle associated with its target for logging.
struct WorkerHandle {
    target: SubscriptionTarget,
    handle: JoinHandle<Result<(), SubscriptionWorkerError>>,
}

fn spawn_worker(
    ws_url: SecretUrl,
    credential: Credential,
    target: SubscriptionTarget,
    max_retries: u32,
    dispatcher_tx: mpsc::Sender<RawLogEvent>,
    events_tx: broadcast::Sender<SubscriptionEvent>,
    shutdown: CancellationToken,
) -> WorkerHandle {
    let worker = SubscriptionWorker::new(ws_url, credential, target.clone(), max_retries);
    let handle = tokio::spawn(async move { worker.run(dispatcher_tx, events_tx, shutdown).await });
    WorkerHandle { target, handle }
}

fn handle_event(event: &SubscriptionEvent) {
    match event {
        SubscriptionEvent::Subscribed { protocol, mention } => {
            info!(
                protocol = %protocol.as_str(),
                mention = %mention,
                "worker subscribed"
            );
        }
        SubscriptionEvent::StreamClosed {
            protocol,
            mention,
            attempt,
        } => {
            warn!(
                protocol = %protocol.as_str(),
                mention = %mention,
                attempt,
                "worker stream closed — will resubscribe"
            );
        }
        SubscriptionEvent::RetryFailed {
            protocol,
            mention,
            attempt,
            error,
        } => {
            warn!(
                protocol = %protocol.as_str(),
                mention = %mention,
                attempt,
                error = %error,
                "worker retry failed"
            );
        }
        SubscriptionEvent::GivingUp {
            protocol,
            mention,
            last_error,
        } => {
            error!(
                protocol = %protocol.as_str(),
                mention = %mention,
                last_error = %last_error,
                "worker exhausted retry budget"
            );
        }
        SubscriptionEvent::ShutdownCompleted { protocol, mention } => {
            info!(
                protocol = %protocol.as_str(),
                mention = %mention,
                "worker shutdown complete"
            );
        }
    }
}

fn push_failure(gave_up: &mut Vec<WorkerFailure>, err: &SubscriptionWorkerError) {
    match err {
        SubscriptionWorkerError::RetriesExhausted {
            protocol,
            mention,
            attempts,
            last_error,
        } => {
            error!(
                protocol = %protocol.as_str(),
                mention = %mention,
                attempts,
                last_error = %last_error,
                "worker gave up after exhausting retries"
            );
            gave_up.push(WorkerFailure {
                protocol: *protocol,
                mention: *mention,
                reason: format!("retries_exhausted after {attempts}: {last_error}"),
            });
        }
    }
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
