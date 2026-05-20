use std::{future::Future, pin::Pin, time::Duration};

use futures_util::StreamExt;
use solana_commitment_config::CommitmentConfig;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::{
    config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    response::{Response, RpcLogsResponse},
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::{
    error::SubscriptionWorkerError,
    infra::rpc::types::{
        raw_log_event::RawLogEvent, subscription_event::SubscriptionEvent,
        subscription_target::SubscriptionTarget,
    },
    utils::redact::redact_api_key,
};

/// Bounds for the worker's internal retry loop.
/// `max_attempts` is read from the environment (`RPC_WORKER_MAX_RETRIES`,
/// default 10) by the caller and passed in.
const INITIAL_BACKOFF_SECS: u64 = 1;
const MAX_BACKOFF_SECS: u64 = 60;

/// A self-contained subscription task.
///
/// Each worker owns its own `PubsubClient` (one WebSocket connection per
/// worker). This wastes a connection per subscription but keeps the worker
/// fully independent: no shared state, no coordination needed with siblings.
///
/// Lifecycle:
///   connect → subscribe → forward events → (on failure) retry with backoff
///                                        → (on budget exhausted) GivingUp
///                                        → (on cancel) ShutdownCompleted
///
/// The worker emits `SubscriptionEvent`s on a broadcast channel so the
/// listener (and any future observer) can track its state.
pub struct SubscriptionWorker {
    ws_url: String,
    target: SubscriptionTarget,
    max_attempts: u32,
}

impl SubscriptionWorker {
    pub fn new(ws_url: String, target: SubscriptionTarget, max_attempts: u32) -> Self {
        Self {
            ws_url,
            target,
            max_attempts,
        }
    }

    /// Run the worker to completion. Terminates when:
    /// - the retry budget is exhausted (→ `GivingUp`),
    /// - the cancel token is triggered (→ `ShutdownCompleted`),
    /// - the dispatcher channel is closed (→ `ShutdownCompleted`).
    pub async fn run(
        self,
        dispatcher_tx: mpsc::Sender<RawLogEvent>,
        events_tx: broadcast::Sender<SubscriptionEvent>,
        shutdown: CancellationToken,
    ) -> Result<(), SubscriptionWorkerError> {
        let SubscriptionWorker {
            ws_url,
            target,
            max_attempts,
        } = self;

        let mut attempt: u32 = 0;
        let mut backoff = INITIAL_BACKOFF_SECS;
        #[allow(unused_assignments)]
        let mut last_error: Option<String> = None;

        loop {
            // Cooperative shutdown check at the top of each attempt.
            if shutdown.is_cancelled() {
                emit(
                    &events_tx,
                    SubscriptionEvent::ShutdownCompleted {
                        protocol: target.protocol,
                        mention: target.mention,
                    },
                );
                return Ok(());
            }

            attempt += 1;

            match connect_and_forward(&ws_url, &target, &dispatcher_tx, &events_tx, &shutdown).await
            {
                ConnectOutcome::ShutdownRequested => {
                    emit(
                        &events_tx,
                        SubscriptionEvent::ShutdownCompleted {
                            protocol: target.protocol,
                            mention: target.mention,
                        },
                    );
                    return Ok(());
                }
                ConnectOutcome::DispatcherClosed => {
                    debug!(
                        protocol = %target.protocol.as_str(),
                        mention = %target.mention,
                        "dispatcher channel closed — worker exiting"
                    );
                    emit(
                        &events_tx,
                        SubscriptionEvent::ShutdownCompleted {
                            protocol: target.protocol,
                            mention: target.mention,
                        },
                    );
                    return Ok(());
                }
                ConnectOutcome::StreamClosed => {
                    emit(
                        &events_tx,
                        SubscriptionEvent::StreamClosed {
                            protocol: target.protocol,
                            mention: target.mention,
                            attempt,
                        },
                    );
                    // On stream closure, reset the attempt counter — the
                    // previous connection was alive long enough to matter.
                    // The retry budget tracks *connection failures*, not
                    // normal churn over a long-lived connection.
                    attempt = 0;
                    backoff = INITIAL_BACKOFF_SECS;
                    // Small pause before resubscribing to avoid hammering
                    // the provider right after it closed us.
                    sleep_or_cancel(Duration::from_secs(1), &shutdown).await;
                    continue;
                }
                ConnectOutcome::Failed(err_msg) => {
                    let redacted = redact_api_key(&err_msg);
                    warn!(
                        protocol = %target.protocol.as_str(),
                        mention = %target.mention,
                        attempt,
                        max = max_attempts,
                        error = %redacted,
                        "worker attempt failed"
                    );
                    emit(
                        &events_tx,
                        SubscriptionEvent::RetryFailed {
                            protocol: target.protocol,
                            mention: target.mention,
                            attempt,
                            error: redacted.clone(),
                        },
                    );
                    last_error = Some(redacted);

                    if attempt >= max_attempts {
                        let msg = last_error.unwrap_or_else(|| "unknown".to_string());
                        emit(
                            &events_tx,
                            SubscriptionEvent::GivingUp {
                                protocol: target.protocol,
                                mention: target.mention,
                                last_error: msg.clone(),
                            },
                        );
                        return Err(SubscriptionWorkerError::RetriesExhausted {
                            protocol: target.protocol,
                            mention: target.mention,
                            attempts: attempt,
                            last_error: msg,
                        });
                    }

                    sleep_or_cancel(Duration::from_secs(backoff), &shutdown).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Outcome of a single connect+forward attempt.
enum ConnectOutcome {
    /// Parent shutdown cancelled mid-attempt.
    ShutdownRequested,
    /// The dispatcher receiver has been dropped — no point continuing.
    DispatcherClosed,
    /// The log stream was closed by the provider. Worth retrying.
    StreamClosed,
    /// Connection or subscription failed. Carries a redactable error string.
    Failed(String),
}

async fn connect_and_forward(
    ws_url: &str,
    target: &SubscriptionTarget,
    dispatcher_tx: &mpsc::Sender<RawLogEvent>,
    events_tx: &broadcast::Sender<SubscriptionEvent>,
    shutdown: &CancellationToken,
) -> ConnectOutcome {
    let pubsub = match PubsubClient::new(ws_url).await {
        Ok(c) => c,
        Err(e) => return ConnectOutcome::Failed(format!("pubsub connect: {e}")),
    };

    let filter = RpcTransactionLogsFilter::Mentions(vec![target.mention.to_string()]);
    let config = RpcTransactionLogsConfig {
        commitment: Some(CommitmentConfig::confirmed()),
    };

    let (stream, unsubscribe) = match pubsub.logs_subscribe(filter, config).await {
        Ok(ok) => ok,
        Err(e) => return ConnectOutcome::Failed(format!("logs_subscribe: {e}")),
    };

    info!(
        protocol = %target.protocol.as_str(),
        mention = %target.mention,
        "subscribed"
    );
    emit(
        events_tx,
        SubscriptionEvent::Subscribed {
            protocol: target.protocol,
            mention: target.mention,
        },
    );

    forward_stream(target, stream, unsubscribe, dispatcher_tx, shutdown).await
}

async fn forward_stream<S, U>(
    target: &SubscriptionTarget,
    mut stream: S,
    unsubscribe: U,
    dispatcher_tx: &mpsc::Sender<RawLogEvent>,
    shutdown: &CancellationToken,
) -> ConnectOutcome
where
    S: StreamExt<Item = Response<RpcLogsResponse>> + Unpin,
    U: FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send,
{
    loop {
        tokio::select! {
            maybe_response = stream.next() => {
                match maybe_response {
                    Some(response) => {
                        let event = RawLogEvent {
                            protocol: target.protocol,
                            signature: response.value.signature,
                            logs: response.value.logs,
                            err: response.value.err.map(Into::into),
                        };

                        match dispatcher_tx.try_send(event) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(dropped)) => {
                                warn!(
                                    protocol = %target.protocol.as_str(),
                                    mention = %target.mention,
                                    signature = %dropped.signature,
                                    "dispatcher channel full — event dropped"
                                );
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                return ConnectOutcome::DispatcherClosed;
                            }
                        }
                    }
                    None => {
                        warn!(
                            protocol = %target.protocol.as_str(),
                            mention = %target.mention,
                            "log stream closed"
                        );
                        return ConnectOutcome::StreamClosed;
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!(
                    protocol = %target.protocol.as_str(),
                    mention = %target.mention,
                    "shutdown requested — unsubscribing"
                );
                unsubscribe().await;
                return ConnectOutcome::ShutdownRequested;
            }
        }
    }
}

/// Sleep for `duration`, but return early if `shutdown` is triggered.
async fn sleep_or_cancel(duration: Duration, shutdown: &CancellationToken) {
    tokio::select! {
        _ = tokio::time::sleep(duration) => {}
        _ = shutdown.cancelled() => {}
    }
}

/// Broadcast send that swallows `SendError` (no active receivers).
/// The worker should keep running even if no one is listening to its events —
/// the dispatcher channel is what matters for its primary job.
fn emit(tx: &broadcast::Sender<SubscriptionEvent>, event: SubscriptionEvent) {
    let _ = tx.send(event);
}
