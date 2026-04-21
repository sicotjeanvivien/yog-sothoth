use futures_util::StreamExt;
use solana_commitment_config::CommitmentConfig;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::{
    client_error::AnyhowError,
    config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    response::{transaction::Signature, Response, RpcLogsResponse},
};
use std::{
    collections::HashSet, future::Future, pin::Pin, str::FromStr, sync::Arc, time::Duration,
};
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use yog_core::domain::Protocol;

use crate::{error::RpcListenerError, utils::redact::redact_api_key};

const MAX_RETRY_ATTEMPTS: u32 = 1000;
const MAX_RETRY_DELAY_SECS: u64 = 60;
const INITIAL_RETRY_DELAY_SECS: u64 = 1;

/// Manages the WebSocket connection to the Solana RPC.
///
/// Responsibilities:
/// - maintain the list of watched protocols
/// - connect to the Solana PubSub WebSocket and subscribe to log events
/// - reconnect automatically on disconnection (exponential backoff)
/// - dispatch incoming signatures to the provided handler
///
/// # Limitations (phase 1)
///
/// Pools added via [`watch`] after [`run`] has started will only be picked up
/// on the next reconnection. Phase 3 will introduce a channel-based mechanism
/// to push watch/unwatch commands into the active loop without reconnecting.
pub(crate) struct RpcListener {
    ws_url: String,
    watched_protocols: Mutex<HashSet<Protocol>>,
}

impl RpcListener {
    pub(crate) fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            watched_protocols: Mutex::new(HashSet::new()),
        }
    }

    /// Add a protocol to the watch list.
    ///
    /// Takes effect on the next (re)connection — not applied to an already
    /// running WebSocket session (phase 1 limitation).
    pub(crate) async fn watch(&self, protocol: Protocol) {
        self.watched_protocols.lock().await.insert(protocol);
    }

    /// Remove a protocol from the watch list.
    ///
    /// Takes effect on the next (re)connection.
    pub(crate) async fn unwatch(&self, protocol: &Protocol) {
        self.watched_protocols.lock().await.remove(protocol);
    }

    /// Start the listener loop with automatic reconnection and graceful shutdown.
    ///
    /// Reconnects on WebSocket disconnection using exponential backoff, up to
    /// [`MAX_RETRY_ATTEMPTS`] consecutive failures. Returns `Ok(())` on clean
    /// shutdown or `Err` if the maximum number of reconnection attempts is exceeded.
    ///
    /// # Cancellation
    ///
    /// The loop exits cleanly when `shutdown` is cancelled — both the
    /// reconnection loop and the active WebSocket session respect the token.
    pub(crate) async fn run(
        &self,
        tx: mpsc::Sender<(Protocol, Signature)>,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut retry_delay = INITIAL_RETRY_DELAY_SECS;
        let mut attempts = 0u32;

        loop {
            info!("connecting to Solana RPC WebSocket: {}", self.ws_url);

            tokio::select! {
                result = self.connect_and_subscribe (tx.clone(), shutdown.clone()) => {
                    attempts += 1;
                    if handle_connection_result(result, attempts, retry_delay)? {
                        return Ok(());
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("RPC listener shutdown requested during connection");
                    return Ok(());
                }
            }

            // Exponential backoff — also cancellable.
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(retry_delay)) => {}
                _ = shutdown.cancelled() => {
                    info!("RPC listener shutdown requested during backoff");
                    return Ok(());
                }
            }

            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY_SECS);
        }
    }

    /// Open a PubSub connection and spawn one subscription task per watched protocol.
    ///
    /// Returns `Ok(())` when all subscription tasks finish normally.
    /// Returns `Err(RpcListenerError::PubSubClient)` if the WebSocket connection fails.
    /// Returns `Err(RpcListenerError::NoProtocolsConfigured)` if no protocols are registered.
    /// Returns `Err(RpcListenerError::AllSubscriptionsFailed)` if every subscription task fails.
    async fn connect_and_subscribe(
        &self,
        tx: mpsc::Sender<(Protocol, Signature)>,
        shutdown: CancellationToken,
    ) -> Result<(), RpcListenerError> {
        let pubsub = connect_pubsub(&self.ws_url).await?;
        let watched_protocols = self.load_watched_protocols().await?;
        info!(
            "connected — subscribing to {} protocol(s)",
            watched_protocols.len()
        );
        let total = watched_protocols.len();

        let handles: Vec<_> = watched_protocols
            .into_iter()
            .map(|protocol| {
                spawn_protocol_subscription(
                    protocol,
                    Arc::clone(&pubsub),
                    tx.clone(),
                    shutdown.clone(),
                )
            })
            .collect();

        join_subscription_handles(handles, total).await
    }

    /// Return the current watch list, or `Err(RpcListenerError::NoProtocolsConfigured)` if empty.
    async fn load_watched_protocols(&self) -> Result<Vec<Protocol>, RpcListenerError> {
        let protocols: Vec<Protocol> = self
            .watched_protocols
            .lock()
            .await
            .iter()
            .cloned()
            .collect();
        if protocols.is_empty() {
            warn!("connected but no protocols to watch");
            return Err(RpcListenerError::NoProtocolsConfigured);
        }
        Ok(protocols)
    }
}

/// Interpret the result of a connection attempt.
///
/// Returns `Ok(true)` on clean stop, `Ok(false)` to retry, or `Err` if the
/// maximum number of attempts has been reached.
fn handle_connection_result(
    result: Result<(), RpcListenerError>,
    attempts: u32,
    retry_delay: u64,
) -> anyhow::Result<bool> {
    match result {
        Ok(()) => {
            info!("RPC listener stopped cleanly");
            Ok(true)
        }
        Err(RpcListenerError::NoProtocolsConfigured) => Ok(false),
        Err(e) => {
            if attempts >= MAX_RETRY_ATTEMPTS {
                return Err(anyhow::anyhow!(
                    "RPC WebSocket unreachable after {attempts} attempts: {}",
                    redact_api_key(&e.to_string())
                ));
            }
            warn!(
                error = %redact_api_key(&e.to_string()),
                attempt = attempts,
                max = MAX_RETRY_ATTEMPTS,
                retry_in = retry_delay,
                "connection issue — reconnecting"
            );
            Ok(false)
        }
    }
}

/// Await all subscription task handles and aggregate failures.
///
/// Individual task failures are logged but do not stop the others.
/// Returns `Err(RpcListenerError::AllSubscriptionsFailed)` only if every task failed.
async fn join_subscription_handles(
    handles: Vec<JoinHandle<Result<(), AnyhowError>>>,
    total: usize,
) -> Result<(), RpcListenerError> {
    let mut failed = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                error!(error = %redact_api_key(&e.to_string()), "subscription task failed");
                failed += 1;
            }
            Err(e) => {
                error!(error = %redact_api_key(&e.to_string()), "subscription task panicked");
                failed += 1;
            }
        }
    }

    if failed == total {
        return Err(RpcListenerError::AllSubscriptionsFailed { count: total });
    }

    Ok(())
}

/// Subscribe to logs for a single protocol and spawn a dispatch task.
///
/// Returns a `JoinHandle` that resolves when the stream closes or shutdown
/// is requested.
fn spawn_protocol_subscription(
    protocol: Protocol,
    pubsub: Arc<PubsubClient>,
    tx: mpsc::Sender<(Protocol, Signature)>,
    shutdown: CancellationToken,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        let program_id = protocol.program_id();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::confirmed()),
        };

        let protocol_name = protocol.as_str();
        let (stream, unsubscribe) = pubsub
            .logs_subscribe(filter, config)
            .await
            .map_err(|e| anyhow::anyhow!("subscribe failed for {protocol_name}: {e}"))?;


        info!(protocol = %protocol_name, program_id = %program_id, "subscribed");

        dispatch_signatures(protocol_name, stream, unsubscribe, protocol, tx, shutdown).await;

        anyhow::Ok(())
    })
}

/// Drive the log stream for a single protocol until the stream closes or shutdown
/// is requested.
///
/// Each incoming signature is dispatched via the `tx` channel to the indexer task.
async fn dispatch_signatures<S, U>(
    protocol_name: &str,
    mut stream: S,
    unsubscribe: U,
    protocol: Protocol,
    tx: mpsc::Sender<(Protocol, Signature)>,
    shutdown: CancellationToken,
) where
    S: StreamExt<Item = Response<RpcLogsResponse>> + Unpin,
    U: FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send,
{
    loop {
        tokio::select! {
            maybe_response = stream.next() => {
                match maybe_response {
                    Some(response) => {
                        let raw_sig = response.value.signature.clone();
                        let logs = &response.value.logs;

                        // Filtre : le programme doit apparaître comme `invoke` dans les logs.
                        // L'address peut être présente via ALT sans que le programme soit réellement
                        // exécuté — on ne veut que les vraies invocations.
                        let program_id_str = protocol.program_id().to_string();
                        let invoke_marker = format!("Program {program_id_str} invoke");
                        if !logs.iter().any(|log| log.starts_with(&invoke_marker)) {
                            tracing::debug!(
                                protocol = %protocol_name,
                                signature = %raw_sig,
                                "skipping — program not invoked (ALT only)"
                            );
                            continue;
                        }

                        // Ignore les txs failed — pas d'effet on-chain donc rien à indexer
                        if response.value.err.is_some() {
                            tracing::debug!(
                                protocol = %protocol_name,
                                signature = %raw_sig,
                                "skipping — transaction failed"
                            );
                            continue;
                        }

                        let signature = match Signature::from_str(&raw_sig) {
                            Ok(sig) => sig,
                            Err(e) => {
                                warn!(error = %e, raw = %raw_sig, "signature invalide");
                                continue;
                            }
                        };

                        tracing::debug!(
                            protocol = %protocol_name,
                            %signature,
                            "📥 websocket emitted signature"
                        );

                        match tx.try_send((protocol.clone(), signature)) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                tracing::warn!(
                                    protocol = %protocol_name,
                                    %signature,
                                    "⚠️  mpsc saturé — signature droppée"
                                );
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                tracing::warn!("channel fermé, arrêt de la subscription");
                                break;
                            }
                        }
                    }
                    None => {
                        warn!("log stream closed for protocol: {protocol_name}");
                        break;
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!("subscription task shutting down for protocol: {protocol_name}");
                unsubscribe().await;
                break;
            }
        }
    }
}

/// Open a WebSocket connection to the Solana PubSub endpoint.
///
/// Returns `Err(RpcListenerError::PubSubClient)` if the connection cannot be established.
async fn connect_pubsub(ws_url: &str) -> Result<Arc<PubsubClient>, RpcListenerError> {
    Ok(Arc::new(PubsubClient::new(ws_url).await.map_err(|e| {
        RpcListenerError::PubSubClient(e.to_string())
    })?))
}
