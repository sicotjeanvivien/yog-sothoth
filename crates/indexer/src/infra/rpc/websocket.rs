use futures_util::StreamExt;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::{
    client_error::AnyhowError,
    config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    response::{Response, RpcLogsResponse},
};
use std::time::Duration;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use yog_core::domain::WatchedPool;

use crate::error::RpcListenerError;

const MAX_RETRY_ATTEMPTS: u32 = 10;
const MAX_RETRY_DELAY_SECS: u64 = 60;
const INITIAL_RETRY_DELAY_SECS: u64 = 1;

/// Manages the WebSocket connection to the Solana RPC.
///
/// Responsibilities:
/// - maintain the list of watched pool addresses
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
    watched_pools: Mutex<Vec<WatchedPool>>,
}

impl RpcListener {
    pub(crate) fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            watched_pools: Mutex::new(Vec::new()),
        }
    }

    /// Add a pool to the watch list.
    ///
    /// Takes effect on the next (re)connection — not applied to an already
    /// running WebSocket session (phase 1 limitation).
    pub(crate) async fn watch(&self, pool: WatchedPool) {
        self.watched_pools.lock().await.push(pool);
    }

    /// Remove a pool from the watch list by address.
    ///
    /// Takes effect on the next (re)connection.
    pub(crate) async fn unwatch(&self, pool_address: &Pubkey) {
        self.watched_pools
            .lock()
            .await
            .retain(|p| &p.pool_address != pool_address);
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
        tx: mpsc::Sender<(WatchedPool, String)>,
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

    /// Open a PubSub connection and spawn one subscription task per watched pool.
    ///
    /// Returns `Ok(())` when all subscription tasks finish normally.
    /// Returns `Err(RpcListenerError::PubSubClient)` if the WebSocket connection fails.
    /// Returns `Err(RpcListenerError::NoPoolsConfigured)` if no pools are registered.
    /// Returns `Err(RpcListenerError::AllSubscriptionsFailed)` if every subscription task fails.
    async fn connect_and_subscribe(
        &self,
        tx: mpsc::Sender<(WatchedPool, String)>,
        shutdown: CancellationToken,
    ) -> Result<(), RpcListenerError> {
        let pubsub = connect_pubsub(&self.ws_url).await?;
        let watched_pools = self.load_watched_pools().await?;
        info!("connected — subscribing to {} pool(s)", watched_pools.len());
        let total = watched_pools.len();

        let handles: Vec<_> = watched_pools
            .into_iter()
            .map(|pool| {
                spawn_pool_subscription(pool, Arc::clone(&pubsub), tx.clone(), shutdown.clone())
            })
            .collect();

        join_subscription_handles(handles, total).await
    }

    /// Return the current watch list, or `Err(RpcListenerError::NoPoolsConfigured)` if empty.
    async fn load_watched_pools(&self) -> Result<Vec<WatchedPool>, RpcListenerError> {
        let pools = self.watched_pools.lock().await.clone();
        if pools.is_empty() {
            warn!("connected but no pools to watch — waiting for subscriptions");
            return Err(RpcListenerError::NoPoolsConfigured);
        }

        Ok(pools)
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
        Err(RpcListenerError::NoPoolsConfigured) => Ok(false),
        Err(e) => {
            if attempts >= MAX_RETRY_ATTEMPTS {
                return Err(anyhow::anyhow!(
                    "RPC WebSocket unreachable after {attempts} attempts: {e}"
                ));
            }
            warn!(
                error = %e,
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
                error!(error = %e, "subscription task failed");
                failed += 1;
            }
            Err(e) => {
                error!(error = %e, "subscription task panicked");
                failed += 1;
            }
        }
    }

    if failed == total {
        return Err(RpcListenerError::AllSubscriptionsFailed { count: total });
    }

    Ok(())
}

/// Subscribe to logs for a single pool and spawn a dispatch task.
///
/// Returns a `JoinHandle` that resolves when the stream closes or shutdown
/// is requested.
fn spawn_pool_subscription(
    pool: WatchedPool,
    pubsub: Arc<PubsubClient>,
    tx: mpsc::Sender<(WatchedPool, String)>,
    shutdown: CancellationToken,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        let pool_address_str = pool.pool_address_str();
        let filter = RpcTransactionLogsFilter::Mentions(vec![pool_address_str.clone()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::confirmed()),
        };

        let (stream, unsubscribe) = pubsub
            .logs_subscribe(filter, config)
            .await
            .map_err(|e| anyhow::anyhow!("subscribe failed for {pool_address_str}: {e}"))?;

        info!("subscribed to pool: {pool_address_str}");

        dispatch_signatures(&pool_address_str, stream, unsubscribe, pool, tx, shutdown).await;

        anyhow::Ok(())
    })
}

/// Drive the log stream for a single pool until the stream closes or shutdown
/// is requested.
///
/// Each incoming signature is dispatched via the `tx` channel to the indexer task.
async fn dispatch_signatures<S, U>(
    pool_address_str: &str,
    mut stream: S,
    unsubscribe: U,
    pool: WatchedPool,
    tx: mpsc::Sender<(WatchedPool, String)>,
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
                        let pool = pool.clone();
                        if let Err(e) = tx.send((pool, response.value.signature)).await {
                            warn!("channel closed, dropping signature: {e}");
                            break;
                        }
                    }
                    None => {
                        warn!("log stream closed for pool: {pool_address_str}");
                        break;
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!("subscription task shutting down for pool: {pool_address_str}");
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
