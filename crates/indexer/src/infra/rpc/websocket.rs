use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tracing::{error, info, warn};

const MAX_RETRY_DELAY_SECS: u64 = 60;
const INITIAL_RETRY_DELAY_SECS: u64 = 1;

/// A watched pool address with its RPC subscription ID.
#[derive(Debug, Clone)]
pub(crate) struct PoolSubscription {
    pub(crate) pool_address: String,
    pub(crate) subscription_id: Option<u64>,
}

/// Manages the WebSocket connection to the Solana RPC.
pub(crate) struct RpcListener {
    rpc_ws_url: String,
    subscriptions: Vec<PoolSubscription>,
}

impl RpcListener {
    pub(crate) fn new(rpc_ws_url: String) -> Self {
        Self {
            rpc_ws_url,
            subscriptions: Vec::new(),
        }
    }

    /// Add a pool address to the watchlist.
    pub(crate) fn watch(&mut self, pool_address: String) {
        self.subscriptions.push(PoolSubscription {
            pool_address,
            subscription_id: None,
        });
    }

    /// Start the listener loop with automatic reconnection.
    pub(crate) async fn run<F, Fut>(&mut self, on_signature: F)
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut retry_delay = INITIAL_RETRY_DELAY_SECS;

        loop {
            info!("connecting to Solana RPC WebSocket: {}", self.rpc_ws_url);

            match self.connect_and_listen(on_signature.clone()).await {
                Ok(_) => {
                    // Clean shutdown — exit the loop
                    info!("RPC listener stopped cleanly");
                    break;
                }
                Err(e) => {
                    warn!(
                        "RPC WebSocket disconnected: {e} — reconnecting in {retry_delay}s"
                    );
                    tokio::time::sleep(Duration::from_secs(retry_delay)).await;

                    // Exponential backoff capped at MAX_RETRY_DELAY_SECS
                    retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY_SECS);
                }
            }
        }
    }

    /// Establish a WebSocket connection, subscribe to all watched pools,
    /// and listen for incoming messages.
    async fn connect_and_listen<F, Fut>(
        &mut self,
        on_signature: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let (ws_stream, _) = connect_async(&self.rpc_ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        info!("connected — subscribing to {} pools", self.subscriptions.len());

        // Subscribe to logs for each watched pool
        for (i, sub) in self.subscriptions.iter_mut().enumerate() {
            let request = json!({
                "jsonrpc": "2.0",
                "id": i + 1,
                "method": "logsSubscribe",
                "params": [
                    { "mentions": [sub.pool_address] },
                    { "commitment": "confirmed" }
                ]
            });

            write
                .send(Message::Text(request.to_string().into()))
                .await?;

            info!("subscribed to pool: {}", sub.pool_address);
        }

        // Reset retry delay on successful connection
        // Listen for incoming messages
        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    if let Some(signature) = extract_signature(&text) {
                        let handler = on_signature.clone();
                        tokio::spawn(async move {
                            handler(signature).await;
                        });
                    }
                }
                Message::Close(_) => {
                    warn!("RPC WebSocket closed by server");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Extract the transaction signature from a logsNotification message.
fn extract_signature(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;

    if value["method"] != "logsNotification" {
        return None;
    }

    value["params"]["result"]["value"]["signature"]
        .as_str()
        .map(|s| s.to_string())
}