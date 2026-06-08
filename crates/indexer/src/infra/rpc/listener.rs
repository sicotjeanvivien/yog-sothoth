use std::{collections::HashSet, sync::Arc};

use solana_pubkey::Pubkey;
use tokio::{
    sync::{Mutex, broadcast, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use yog_core::domain::Protocol;

use crate::{
    application::workers::SubscriptionWorker,
    error::{RpcListenerError, SubscriptionWorkerError},
    infra::rpc::{RawLogEvent, SubscriptionEvent, SubscriptionTarget},
};

/// Default size of the broadcast channel carrying `SubscriptionEvent`s.
/// Oversized on purpose — with a handful of workers emitting occasional
/// events, we never want the listener to drop telemetry because it lagged
/// for a few milliseconds.
const EVENTS_CHANNEL_CAPACITY: usize = 256;

/// Orchestrator for a pool of `SubscriptionWorker`s.
///
/// Responsibilities kept deliberately minimal:
/// - build the list of `SubscriptionTarget`s from watched protocols and pools
/// - spawn one `SubscriptionWorker` per target
/// - consume their `SubscriptionEvent`s (log, metrics, tracking)
/// - escalate to the Daemon when *all* workers have given up
///
/// The listener does NOT:
/// - manage retries (each worker owns its retry budget)
/// - force a global reconnect when one worker dies (siblings keep running)
/// - respawn dead workers (future work — see roadmap)
pub struct RpcListener {
    ws_url: String,
    watched_protocols: Mutex<HashSet<Protocol>>,
    watched_pools: Mutex<HashSet<(Protocol, Pubkey)>>,
    worker_max_retries: u32,
    /// Selects between protocol-centric (subscribe to programs) and
    /// pool-centric (subscribe to individual pool accounts) indexing.
    ///
    /// TODO(post-RPC-upgrade): remove pool-centric mode and this flag.
    /// Pool-centric exists only because the free public RPC cannot sustain
    /// the full protocol firehose. Once we migrate to a paid RPC (Helius /
    /// Shyft / Triton, see roadmap), protocol-centric becomes the only mode
    /// and `target_pools` + this flag should be deleted.
    mode_protocol_centric: bool,
}

impl RpcListener {
    pub fn new(ws_url: String, worker_max_retries: u32, mode_protocol_centric: bool) -> Self {
        Self {
            ws_url,
            watched_protocols: Mutex::new(HashSet::new()),
            watched_pools: Mutex::new(HashSet::new()),
            worker_max_retries,
            mode_protocol_centric,
        }
    }

    pub async fn _watch(&self, protocol: Protocol) {
        self.watched_protocols.lock().await.insert(protocol);
    }

    pub async fn _unwatch(&self, protocol: &Protocol) {
        self.watched_protocols.lock().await.remove(protocol);
    }

    pub async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.watched_pools
            .lock()
            .await
            .insert((protocol, pool_address));
    }

    /// Spawn workers, supervise them, and return when they're all done.
    ///
    /// Returns `Err(AllWorkersGaveUp)` with per-worker details when every
    /// spawned worker has exhausted its retry budget. Returns `Ok(())` when
    /// the shutdown token was cancelled before that happened.
    pub async fn run(
        self: Arc<Self>,
        dispatcher_tx: mpsc::Sender<RawLogEvent>,
        shutdown: CancellationToken,
    ) -> Result<(), RpcListenerError> {
        let targets = self.build_subscription_targets().await?;
        let total = targets.len();

        let (events_tx, _events_rx) =
            broadcast::channel::<SubscriptionEvent>(EVENTS_CHANNEL_CAPACITY);

        info!(count = total, "spawning subscription workers");

        let mut handles: Vec<WorkerHandle> = targets
            .into_iter()
            .map(|target| {
                spawn_worker(
                    self.ws_url.clone(),
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
        let targets = if self.mode_protocol_centric {
            self.target_protocols().await
        } else {
            self.target_pools().await
        };

        if targets.is_empty() {
            return Err(RpcListenerError::NoSubscriptionTargets);
        }
        Ok(targets)
    }

    async fn target_protocols(&self) -> Vec<SubscriptionTarget> {
        self.watched_protocols
            .lock()
            .await
            .iter()
            .cloned()
            .map(|protocol| {
                let program_id = protocol.program_id();
                SubscriptionTarget::new(protocol, program_id)
            })
            .collect()
    }

    async fn target_pools(&self) -> Vec<SubscriptionTarget> {
        self.watched_pools
            .lock()
            .await
            .iter()
            .cloned()
            .map(|(protocol, pool)| SubscriptionTarget::new(protocol, pool))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Supervision helpers
// ---------------------------------------------------------------------------

/// Per-worker failure detail — bubbled up in `AllWorkersGaveUp`.
#[derive(Debug, Clone)]
pub struct WorkerFailure {
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
    ws_url: String,
    target: SubscriptionTarget,
    max_retries: u32,
    dispatcher_tx: mpsc::Sender<RawLogEvent>,
    events_tx: broadcast::Sender<SubscriptionEvent>,
    shutdown: CancellationToken,
) -> WorkerHandle {
    let worker = SubscriptionWorker::new(ws_url, target.clone(), max_retries);
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
