use futures_util::StreamExt;
use solana_commitment_config::CommitmentConfig;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

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
    http_url: String,
    /// Snapshot-based: locked briefly at each (re)connection, then released.
    watched_pools: Mutex<Vec<String>>,
}

impl RpcListener {
    pub(crate) fn new(ws_url: String, http_url: String) -> Self {
        Self {
            ws_url,
            http_url,
            watched_pools: Mutex::new(Vec::new()),
        }
    }

    /// Add a pool address to the watch list.
    ///
    /// Takes effect on the next (re)connection — not applied to an already
    /// running WebSocket session (phase 1 limitation).
    pub(crate) async fn watch(&self, address: String) {
        self.watched_pools.lock().await.push(address);
    }

    /// Remove a pool address from the watch list.
    ///
    /// Takes effect on the next (re)connection.
    pub(crate) async fn unwatch(&self, address: String) {
        self.watched_pools.lock().await.retain(|a| a != &address);
    }

    /// Start the listener loop with automatic reconnection and graceful shutdown.
    ///
    /// Reconnects on WebSocket disconnection using exponential backoff, up to
    /// [`MAX_RETRY_ATTEMPTS`] consecutive failures. Returns `Ok(())` on clean
    /// shutdown (cancellation token fired) or after a successful connection
    /// cycle. Returns `Err` if the maximum number of reconnection attempts is
    /// exceeded.
    ///
    /// # Cancellation
    ///
    /// The loop exits cleanly when `shutdown` is cancelled — both the
    /// reconnection loop and the active WebSocket session respect the token.
    pub(crate) async fn run<F, Fut>(
        &self,
        on_signature: F,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()>
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut retry_delay = INITIAL_RETRY_DELAY_SECS;
        let mut attempts = 0u32;

        loop {
            info!("connecting to Solana RPC WebSocket: {}", self.ws_url);

            tokio::select! {
                // Drive the connection attempt and the active listen session.
                result = self.connect_and_listen(on_signature.clone(), shutdown.clone()) => {
                    match result {
                        Ok(()) => {
                            // Clean exit from connect_and_listen (e.g. shutdown token fired
                            // inside the session, or all streams ended normally).
                            info!("RPC listener stopped cleanly");
                            return Ok(());
                        }
                        Err(e) => {
                            attempts += 1;
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
                                "RPC WebSocket disconnected — reconnecting"
                            );
                        }
                    }
                }
                // Honour a shutdown request that arrives while we are waiting
                // for connect_and_listen (e.g. connection is hanging).
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

    /// Open a PubSub connection and subscribe to all watched pools.
    ///
    /// Returns `Ok(())` when all subscription tasks finish normally or when
    /// `shutdown` is cancelled. Returns `Err` on connection or subscription
    /// failure.
    async fn connect_and_listen<F, Fut>(
        &self,
        on_signature: F,
        shutdown: CancellationToken,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let pubsub = Arc::new(PubsubClient::new(&self.ws_url).await?);

        // Lock, clone, release immediately — the mutex is never held across
        // an await point in the hot path.
        let addresses = self.watched_pools.lock().await.clone();
        info!("connected — subscribing to {} pool(s)", addresses.len());

        let mut handles = Vec::new();

        for address in addresses {
            let pubsub = Arc::clone(&pubsub);
            let on_signature = on_signature.clone();
            let shutdown = shutdown.clone();

            let handle = tokio::spawn(async move {
                let filter = RpcTransactionLogsFilter::Mentions(vec![address.clone()]);
                let config = RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::confirmed()),
                };

                // Propagate subscription errors instead of panicking —
                // a panic inside tokio::spawn is silently swallowed.
                let (mut stream, unsubscribe) = pubsub
                    .logs_subscribe(filter, config)
                    .await
                    .map_err(|e| anyhow::anyhow!("subscribe failed for {address}: {e}"))?;

                info!("subscribed to pool: {address}");

                loop {
                    tokio::select! {
                        maybe_response = stream.next() => {
                            match maybe_response {
                                Some(response) => {
                                    let signature = response.value.signature;
                                    let handler = on_signature.clone();
                                    tokio::spawn(async move {
                                        handler(signature).await;
                                    });
                                }
                                // Stream ended — connection was dropped on the server side.
                                None => {
                                    warn!("log stream closed for pool: {address}");
                                    break;
                                }
                            }
                        }
                        _ = shutdown.cancelled() => {
                            info!("subscription task shutting down for pool: {address}");
                            // Explicitly unsubscribe before dropping — sends the
                            // unsubscribe message to the server for a clean teardown.
                            unsubscribe().await;
                            break;
                        }
                    }
                }

                anyhow::Ok(())
            });

            handles.push(handle);
        }

        // Wait for all subscription tasks and surface any errors.
        for handle in handles {
            match handle.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => error!(error = %e, "subscription task failed"),
                // JoinError — the task panicked.
                Err(e) => error!(error = %e, "subscription task panicked"),
            }
        }

        Ok(())
    }
}
