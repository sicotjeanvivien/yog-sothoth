use futures_util::StreamExt;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::{
    config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    response::{Response, RpcLogsResponse},
};
use std::time::Duration;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use yog_core::domain::WatchedPool;

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

    /// Add a pool address to the watch list.
    ///
    /// Takes effect on the next (re)connection — not applied to an already
    /// running WebSocket session (phase 1 limitation).
    pub(crate) async fn watch(&self, pool: WatchedPool) {
        self.watched_pools.lock().await.push(pool);
    }

    /// Remove a pool address from the watch list.
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
        index_transaction: F,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()>
    where
        F: Fn(WatchedPool, String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut retry_delay = INITIAL_RETRY_DELAY_SECS;
        let mut attempts = 0u32;

        loop {
            info!("connecting to Solana RPC WebSocket: {}", self.ws_url);

            tokio::select! {
                // Drive the connection attempt and the active listen session.
                result = self.subscriber_pool(index_transaction.clone(), shutdown.clone()) => {
                    match result {
                        Ok(()) => {
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

    async fn subscriber_pool<F, Fut>(
        &self,
        index_transaction: F,
        shutdown: CancellationToken,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(WatchedPool, String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let pubsub = Arc::new(PubsubClient::new(&self.ws_url).await?);
        let watched_pools = self.watched_pools.lock().await.clone();
        info!("connected — subscribing to {} pool(s)", watched_pools.len());

        let handles: Vec<_> = watched_pools
            .into_iter()
            .map(|pool| {
                listen_pool(
                    pool,
                    Arc::clone(&pubsub),
                    index_transaction.clone(),
                    shutdown.clone(),
                )
            })
            .collect();

        for handle in handles {
            match handle.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => error!(error = %e, "subscription task failed"),
                Err(e) => error!(error = %e, "subscription task panicked"),
            }
        }

        Ok(())
    }
}

fn listen_pool<F, Fut>(
    pool: WatchedPool,
    pubsub: Arc<PubsubClient>,
    index_transaction: F,
    shutdown: CancellationToken,
) -> JoinHandle<anyhow::Result<()>>
where
    F: Fn(WatchedPool, String) -> Fut + Send + Sync + Clone + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
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

        dispatch_signatures(
            &pool_address_str,
            stream,
            unsubscribe,
            pool,
            index_transaction,
            shutdown,
        )
        .await;

        anyhow::Ok(())
    })
}

async fn dispatch_signatures<F, Fut, S, U>(
    pool_address_str: &str,
    mut stream: S,
    unsubscribe: U,
    pool: WatchedPool,
    index_transaction: F,
    shutdown: CancellationToken,
) where
    F: Fn(WatchedPool, String) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
    S: StreamExt<Item = Response<RpcLogsResponse>> + Unpin,
    U: FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send,
{
    loop {
        tokio::select! {
            maybe_response = stream.next() => {
                match maybe_response {
                    Some(response) => {
                        let pool = pool.clone();
                        let index_transaction = index_transaction.clone();
                        tokio::spawn(async move {
                            index_transaction(pool, response.value.signature).await;
                        });
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
