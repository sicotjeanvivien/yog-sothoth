use futures_util::StreamExt;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_commitment_config::CommitmentConfig;
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

const MAX_RETRY_DELAY_SECS: u64 = 60;
const INITIAL_RETRY_DELAY_SECS: u64 = 1;

/// Manages the WebSocket connection to the Solana RPC.
pub(crate) struct RpcListener {
    ws_url: String,
    http_url: String,
    pool_addresses: Vec<String>,
}

impl RpcListener {
    pub(crate) fn new(ws_url: String, http_url: String) -> Self {
        Self {
            ws_url,
            http_url,
            pool_addresses: Vec::new(),
        }
    }

    /// Add a pool address to the watchlist.
    pub(crate) fn watch(&mut self, pool_address: String) {
        self.pool_addresses.push(pool_address);
    }

    /// Start the listener loop with automatic reconnection.
    pub(crate) async fn run<F, Fut>(&self, on_signature: F)
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut retry_delay = INITIAL_RETRY_DELAY_SECS;

        loop {
            info!("connecting to Solana RPC WebSocket: {}", self.ws_url);

            match self.connect_and_listen(on_signature.clone()).await {
                Ok(_) => {
                    info!("RPC listener stopped cleanly");
                    break;
                }
                Err(e) => {
                    warn!("RPC WebSocket disconnected: {e} — reconnecting in {retry_delay}s");
                    tokio::time::sleep(Duration::from_secs(retry_delay)).await;
                    retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY_SECS);
                }
            }
        }
    }

    async fn connect_and_listen<F, Fut>(
        &self,
        on_signature: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let pubsub = Arc::new(PubsubClient::new(&self.ws_url).await?);
        let rpc = Arc::new(RpcClient::new(self.http_url.clone()));

        info!(
            "connected — subscribing to {} pools",
            self.pool_addresses.len()
        );

        let mut handles = Vec::new();

        for address in &self.pool_addresses {
            let pubsub = Arc::clone(&pubsub);
            let on_signature = on_signature.clone();
            let address = address.clone();

            let handle = tokio::spawn(async move {
                let filter = RpcTransactionLogsFilter::Mentions(vec![address.clone()]);
                let config = RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::confirmed()),
                };

                let (mut stream, _unsubscribe) = pubsub
                    .logs_subscribe(filter, config)
                    .await
                    .expect("failed to subscribe to logs");

                info!("subscribed to pool: {address}");

                while let Some(response) = stream.next().await {
                    let signature = response.value.signature;
                    let handler = on_signature.clone();
                    tokio::spawn(async move {
                        handler(signature).await;
                    });
                }
            });

            handles.push(handle);
        }

        // Wait for all subscription tasks
        for handle in handles {
            handle.await?;
        }

        Ok(())
    }
}
