use solana_commitment_config::CommitmentConfig;
use solana_pubkey::pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::sync::Arc;
use tokio_retry::{strategy::FixedInterval, Retry};
use tracing::{debug, error, info, warn};
use yog_core::domain::{PoolMetric, PoolMetricRepository, SwapEventRepository};
use yog_core::CoreResult;
use yog_core::{
    amm::{
        common::{imbalance, spot_price},
        damm_v2::net_price_impact,
    },
    domain::SwapEvent,
    protocols::{meteora::damm_v2::DammV2, PoolIndexer},
};

/// Core pipeline — receives a signature, fetches the full transaction,
/// dispatches to the appropriate protocol handler.
pub(crate) struct IndexerService {
    rpc_client: Arc<RpcClient>,
    swap_event_repo: Arc<dyn SwapEventRepository + Send + Sync>,
    pool_metric_repo: Arc<dyn PoolMetricRepository + Send + Sync>,
}

impl IndexerService {
    pub(crate) fn new(
        rpc_client: Arc<RpcClient>,
        swap_event_repo: Arc<dyn SwapEventRepository + Send + Sync>,
        pool_metric_repo: Arc<dyn PoolMetricRepository + Send + Sync>,
    ) -> Self {
        Self {
            rpc_client,
            swap_event_repo,
            pool_metric_repo,
        }
    }

    /// Handle a transaction signature received from the WebSocket.
    pub(crate) async fn handle_signature(&self, signature: String) {
        info!("received signature: {signature}");

        match self.fetch_transaction(&signature).await {
            Ok(Some(tx)) => {
                let damm_v2_proto =
                    DammV2::new(pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"));
                match damm_v2_proto.parse_swap(&tx) {
                    Ok(swap) => {
                        info!(
                            signature = %swap.signature,
                            amount_in = swap.amount_in,
                            amount_out = swap.amount_out,
                            "swap parsed"
                        );

                        if let Err(e) = self.swap_event_repo.insert(&swap).await {
                            error!("failed to insert swap_event: {e}");
                            return;
                        }

                        match self.compute_metrics(&swap) {
                            Ok(metric) => {
                                if let Err(e) = self.pool_metric_repo.insert(&metric).await {
                                    error!("failed to insert pool_metric: {e}");
                                }
                            }
                            Err(e) => error!("failed to compute metrics: {e}"),
                        }
                    }
                    Err(e) => {
                        debug!("skipping non-swap transaction {signature}: {e}");
                    }
                }
            }
            Ok(None) => warn!("transaction not found: {signature}"),
            Err(e) => error!("failed to fetch transaction {signature}: {e}"),
        }
    }

    /// Compute and log AMM metrics from a parsed swap.
    fn compute_and_log_metrics(&self, swap: &SwapEvent) {
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
    ) -> anyhow::Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
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
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    }

    fn compute_metrics(&self, swap: &SwapEvent) -> CoreResult<PoolMetric> {
        let reserve_a = swap.reserve_a_after as u128;
        let reserve_b = swap.reserve_b_after as u128;
        let amount_in = swap.amount_in as u128;

        let price_q64 = spot_price(reserve_a, reserve_b)?;
        let imbalance_bps = imbalance(reserve_a, reserve_b).ok().map(|v| v as i32);
        let price_impact_bps = net_price_impact(reserve_a, reserve_b, amount_in, 25)
            .ok()
            .map(|v| v as i32);

        let price_display = price_q64 as f64 / (1u128 << 64) as f64;
        info!(
            price_q64,
            price_display, imbalance_bps, price_impact_bps, "metrics computed"
        );

        Ok(PoolMetric {
            pool_address: swap.pool_address,
            signature: swap.signature.clone(),
            reserve_a: swap.reserve_a_after,
            reserve_b: swap.reserve_b_after,
            price_q64,
            price_impact_bps,
            imbalance_bps,
            current_fee_bps: swap.fee_bps.map(|f| f as i32),
            fees_collected_a: None,
            fees_collected_b: None,
            volume_a: Some(swap.amount_in),
            volume_b: Some(swap.amount_out),
            active_bin_id: None,
            bin_step: None,
            timestamp: swap.timestamp,
        })
    }
}
