use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::sync::Arc;
use tokio_retry::{strategy::FixedInterval, Retry};
use tracing::{error, info, warn};

/// Core pipeline — receives a signature, fetches the full transaction,
/// dispatches to the appropriate protocol handler.
pub(crate) struct IndexerService {
    rpc_client: Arc<RpcClient>,
}

impl IndexerService {
    pub(crate) fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    /// Handle a transaction signature received from the WebSocket.
    pub(crate) async fn handle_signature(&self, signature: String) {
        info!("received signature: {signature}");

        match self.fetch_transaction(&signature).await {
            Ok(Some(tx)) => {
                info!("fetched transaction: slot={:?}", tx.slot,);
                // TODO: dispatch to PoolIndexer (Phase 1)
                // TODO: compute AMM metrics (Phase 1)
                // TODO: write to DB (Phase 1)
            }
            Ok(None) => {
                warn!("transaction not found: {signature}");
            }
            Err(e) => {
                error!("failed to fetch transaction {signature}: {e}");
            }
        }
    }

    /// Fetch a confirmed transaction by signature from the RPC.
    async fn fetch_transaction(
        &self,
        signature: &str,
    ) -> Result<Option<EncodedConfirmedTransactionWithStatusMeta>, Box<dyn std::error::Error>> {
        let sig = signature.parse()?;

        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        let strategy = FixedInterval::from_millis(500).take(5);

        let result = Retry::spawn(strategy, || async {
            self.rpc_client
                .get_transaction_with_config(&sig, config)
                .await
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(tx) => Ok(Some(tx)),
            Err(e) if e.contains("null") => {
                warn!("transaction not available after retries: {signature}");
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
}
