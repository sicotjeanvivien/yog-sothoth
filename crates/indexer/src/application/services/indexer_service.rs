use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::sync::Arc;
use tokio_retry::{strategy::FixedInterval, Retry};
use tracing::{error, info, warn};
use yog_core::amm::common::{imbalance, spot_price};
use yog_core::amm::damm_v2::net_price_impact;
use yog_core::protocols::meteora::damm_v2::DammV2;
use yog_core::types::DammV2SwapResult;

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
                match DammV2::parse_swap(&tx, "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j") {
                    Ok(Some(swap)) => {
                        info!(
                            signature = %swap.signature,
                            amount_in = swap.amount_in,
                            amount_out = swap.amount_out,
                            token_in = %swap.token_in_mint,
                            token_out = %swap.token_out_mint,
                            "swap parsed"
                        );
                        self.compute_and_log_metrics(&swap);
                    }
                    Ok(None) => {} // not a DAMM v2 swap — ignore
                    Err(e) => {
                        error!("failed to parse swap {signature}: {e}");
                    }
                }
            }
            Ok(None) => {
                warn!("transaction not found: {signature}");
            }
            Err(e) => {
                error!("failed to fetch transaction {signature}: {e}");
            }
        }
    }

    /// Compute and log AMM metrics from a parsed swap.
    fn compute_and_log_metrics(&self, swap: &DammV2SwapResult) {
        let reserve_a = swap.reserve_a_after as u128;
        let reserve_b = swap.reserve_b_after as u128;
        let amount_in = swap.amount_in as u128;

        match spot_price(reserve_a, reserve_b) {
            Ok(price_q64) => {
                let price_display = price_q64 as f64 / (1u128 << 64) as f64;
                info!(price_q64, price_display, "spot price");
            }
            Err(e) => error!("spot_price: {e}"),
        }

        match imbalance(reserve_a, reserve_b) {
            Ok(bps) => info!(imbalance_bps = bps, "pool imbalance"),
            Err(e) => error!("imbalance: {e}"),
        }

        // fee hardcoded at 25 bps — will be parsed from tx in next iteration
        match net_price_impact(reserve_a, reserve_b, amount_in, 25) {
            Ok(bps) => info!(price_impact_bps = bps, "net price impact"),
            Err(e) => error!("net_price_impact: {e}"),
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
