//! Étage bas du pipeline d'ingestion : maintient les connexions WebSocket
//! Solana et émet les événements bruts.
//!
//! Responsabilité unique : garder les subscriptions vivantes et pousser tout
//! ce qui arrive dans le channel aval. Aucun filtrage métier — ça, c'est
//! le rôle du [`SignatureDispatcher`](super::dispatcher::SignatureDispatcher).

use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc, time::Duration};

use futures_util::StreamExt;
use solana_commitment_config::CommitmentConfig;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::{
    client_error::AnyhowError,
    config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    response::{Response, RpcLogsResponse},
};
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use yog_core::domain::Protocol;

use crate::{error::RpcListenerError, utils::redact::redact_api_key};

use super::types::RawLogEvent;

const MAX_RETRY_ATTEMPTS: u32 = 1000;
const MAX_RETRY_DELAY_SECS: u64 = 60;
const INITIAL_RETRY_DELAY_SECS: u64 = 1;

pub struct RpcListener {
    ws_url: String,
    watched_protocols: Mutex<HashSet<Protocol>>,
}

impl RpcListener {
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            watched_protocols: Mutex::new(HashSet::new()),
        }
    }

    pub async fn watch(&self, protocol: Protocol) {
        self.watched_protocols.lock().await.insert(protocol);
    }

    pub async fn unwatch(&self, protocol: &Protocol) {
        self.watched_protocols.lock().await.remove(protocol);
    }

    pub async fn run(
        &self,
        tx: mpsc::Sender<RawLogEvent>,
        shutdown: CancellationToken,
    ) -> Result<(), RpcListenerError> {
        let mut retry_delay = INITIAL_RETRY_DELAY_SECS;
        let mut attempts = 0u32;

        loop {
            info!(url = %redact_api_key(&self.ws_url), "connecting to Solana RPC WebSocket");

            tokio::select! {
                result = self.connect_and_subscribe(tx.clone(), shutdown.clone()) => {
                    match result {
                        Ok(()) => {
                            info!("RPC listener stopped cleanly");
                            return Ok(());
                        }
                        Err(RpcListenerError::NoProtocolsConfigured) => {
                            warn!("no protocols configured — listener idle");
                            return Err(RpcListenerError::NoProtocolsConfigured);
                        }
                        Err(e) => {
                            attempts += 1;
                            if attempts >= MAX_RETRY_ATTEMPTS {
                                return Err(RpcListenerError::MaxRetriesExceeded {
                                    attempts,
                                    message: redact_api_key(&e.to_string()),
                                });
                            }
                            warn!(
                                error = %redact_api_key(&e.to_string()),
                                attempt = attempts,
                                max = MAX_RETRY_ATTEMPTS,
                                retry_in_secs = retry_delay,
                                "connection issue — reconnecting"
                            );
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested during connection");
                    return Ok(());
                }
            }

            // Backoff annulable.
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(retry_delay)) => {}
                _ = shutdown.cancelled() => {
                    info!("shutdown requested during backoff");
                    return Ok(());
                }
            }

            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY_SECS);
        }
    }

    /// Ouvre la connexion PubSub et spawn une task par protocole.
    async fn connect_and_subscribe(
        &self,
        tx: mpsc::Sender<RawLogEvent>,
        shutdown: CancellationToken,
    ) -> Result<(), RpcListenerError> {
        let pubsub = connect_pubsub(&self.ws_url).await?;
        let watched_protocols = self.load_watched_protocols().await?;
        let total = watched_protocols.len();

        info!(count = total, "connected — subscribing to protocols");

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

    async fn load_watched_protocols(&self) -> Result<Vec<Protocol>, RpcListenerError> {
        let protocols: Vec<Protocol> = self
            .watched_protocols
            .lock()
            .await
            .iter()
            .cloned()
            .collect();

        if protocols.is_empty() {
            return Err(RpcListenerError::NoProtocolsConfigured);
        }
        Ok(protocols)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn connect_pubsub(ws_url: &str) -> Result<Arc<PubsubClient>, RpcListenerError> {
    PubsubClient::new(ws_url)
        .await
        .map(Arc::new)
        .map_err(|e| RpcListenerError::PubSubClient(e.to_string()))
}

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

fn spawn_protocol_subscription(
    protocol: Protocol,
    pubsub: Arc<PubsubClient>,
    tx: mpsc::Sender<RawLogEvent>,
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

        forward_raw_events(protocol_name, stream, unsubscribe, protocol, tx, shutdown).await;

        anyhow::Ok(())
    })
}

/// Pousse chaque log reçu dans le channel aval, sans aucun filtrage.
///
/// Seule règle : si le channel est saturé, on drop — le pipeline entier ne doit
/// pas se bloquer à cause d'un consumer lent. Compté côté Dispatcher.
async fn forward_raw_events<S, U>(
    protocol_name: &str,
    mut stream: S,
    unsubscribe: U,
    protocol: Protocol,
    tx: mpsc::Sender<RawLogEvent>,
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
                        println!("{:?}",response);
                        let event = RawLogEvent {
                            protocol: protocol.clone(),
                            signature: response.value.signature,
                            logs: response.value.logs,
                            err: response.value.err.map(Into::into),
                        };

                        match tx.try_send(event) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(dropped)) => {
                                warn!(
                                    protocol = %protocol_name,
                                    signature = %dropped.signature,
                                    "dispatcher channel full — event dropped"
                                );
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                debug!("dispatcher channel closed — stopping subscription");
                                break;
                            }
                        }
                    }
                    None => {
                        warn!(protocol = %protocol_name, "log stream closed");
                        break;
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!(protocol = %protocol_name, "subscription shutting down");
                unsubscribe().await;
                break;
            }
        }
    }
}
