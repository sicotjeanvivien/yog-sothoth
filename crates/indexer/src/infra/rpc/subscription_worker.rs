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
use yog_bootstrap::SecretUrl;

use crate::{
    error::SubscriptionWorkerError,
    infra::{
        Credential,
        rpc::{RawLogEvent, SubscriptionEvent, SubscriptionTarget},
    },
};

/// Backoff bounds of the worker's retry loop; the attempt budget is `RPC_WORKER_MAX_RETRIES`.
const INITIAL_BACKOFF_SECS: u64 = 1;
const MAX_BACKOFF_SECS: u64 = 60;

/// One `logsSubscribe` subscription on its own WebSocket, since the method takes one pubkey:
/// connect, subscribe, forward, retry with backoff until the budget is spent.
pub(crate) struct SubscriptionWorker {
    ws_url: SecretUrl,
    /// The header this endpoint authenticates with, if any, validated by the listener.
    credential: Credential,
    target: SubscriptionTarget,
    max_attempts: u32,
}

impl SubscriptionWorker {
    pub(crate) fn new(
        ws_url: SecretUrl,
        credential: Credential,
        target: SubscriptionTarget,
        max_attempts: u32,
    ) -> Self {
        Self {
            ws_url,
            credential,
            target,
            max_attempts,
        }
    }

    /// Run the worker to completion: `Err` when the retry budget is exhausted, `Ok(())` on
    /// cancellation or a closed dispatcher channel.
    pub(crate) async fn run(
        self,
        dispatcher_tx: mpsc::Sender<RawLogEvent>,
        events_tx: broadcast::Sender<SubscriptionEvent>,
        shutdown: CancellationToken,
    ) -> Result<(), SubscriptionWorkerError> {
        let SubscriptionWorker {
            ws_url,
            credential,
            target,
            max_attempts,
        } = self;

        let mut attempt: u32 = 0;
        let mut backoff = INITIAL_BACKOFF_SECS;
        #[allow(unused_assignments)]
        let mut last_error: Option<String> = None;

        loop {
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

            match connect_and_forward(
                &ws_url,
                &credential,
                &target,
                &dispatcher_tx,
                &events_tx,
                &shutdown,
            )
            .await
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
                    // A stream that closed was alive: the budget counts connection failures,
                    // not churn.
                    attempt = 0;
                    backoff = INITIAL_BACKOFF_SECS;
                    // A short pause, not to hammer the provider that just closed us.
                    sleep_or_cancel(Duration::from_secs(1), &shutdown).await;
                    continue;
                }
                ConnectOutcome::Failed(err_msg) => {
                    warn!(
                        protocol = %target.protocol.as_str(),
                        mention = %target.mention,
                        attempt,
                        max = max_attempts,
                        error = %err_msg,
                        "worker attempt failed"
                    );
                    emit(
                        &events_tx,
                        SubscriptionEvent::RetryFailed {
                            protocol: target.protocol,
                            mention: target.mention,
                            attempt,
                            error: err_msg.clone(),
                        },
                    );
                    last_error = Some(err_msg);

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
    ws_url: &SecretUrl,
    credential: &Credential,
    target: &SubscriptionTarget,
    dispatcher_tx: &mpsc::Sender<RawLogEvent>,
    events_tx: &broadcast::Sender<SubscriptionEvent>,
    shutdown: &CancellationToken,
) -> ConnectOutcome {
    // The handshake request, not the bare URL: it is what carries a header credential.
    let request = match credential.ws_request(ws_url) {
        Ok(request) => request,
        Err(scrubbed) => return ConnectOutcome::Failed(scrubbed),
    };

    let pubsub = match PubsubClient::new(request).await {
        Ok(c) => c,
        Err(e) => return ConnectOutcome::Failed(ws_url.scrub(&format!("pubsub connect: {e}"))),
    };

    let filter = RpcTransactionLogsFilter::Mentions(vec![target.mention.to_string()]);
    let config = RpcTransactionLogsConfig {
        commitment: Some(CommitmentConfig::confirmed()),
    };

    let (stream, unsubscribe) = match pubsub.logs_subscribe(filter, config).await {
        Ok(ok) => ok,
        Err(e) => return ConnectOutcome::Failed(ws_url.scrub(&format!("logs_subscribe: {e}"))),
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

/// Broadcast send that ignores a missing receiver: the dispatcher channel is the worker's job.
fn emit(tx: &broadcast::Sender<SubscriptionEvent>, event: SubscriptionEvent) {
    let _ = tx.send(event);
}
