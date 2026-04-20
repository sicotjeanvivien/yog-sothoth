use chrono::Utc;
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::sync::Arc;
use tokio_retry::{strategy::FixedInterval, Retry};
use tracing::{debug, info, warn};
use yog_core::{
    amm::{
        common::{imbalance, spot_price},
        damm_v2::net_price_impact,
    },
    domain::{
        LiquidityEvent, LiquidityEventRepository, Pool, PoolMetric, PoolMetricRepository,
        PoolRepository, Protocol, SwapEvent, SwapEventRepository,
    },
    protocols::{
        meteora::{MeteoraDammV1, MeteoraDammV2, MeteoraDlmm},
        PoolIndexer,
    },
    CoreResult,
};

/// Core pipeline — receives a signature, fetches the full transaction,
/// dispatches to the appropriate protocol handler.
pub(crate) struct IndexerService {
    liquidity_event_repo: Arc<dyn LiquidityEventRepository>,
    pool_repo: Arc<dyn PoolRepository>,
    pool_metric_repo: Arc<dyn PoolMetricRepository>,
    rpc_client: Arc<RpcClient>,
    swap_event_repo: Arc<dyn SwapEventRepository>,
}

impl IndexerService {
    pub(crate) fn new(
        liquidity_event_repo: Arc<dyn LiquidityEventRepository>,
        pool_repo: Arc<dyn PoolRepository>,
        pool_metric_repo: Arc<dyn PoolMetricRepository>,
        rpc_client: Arc<RpcClient>,
        swap_event_repo: Arc<dyn SwapEventRepository>,
    ) -> Self {
        Self {
            liquidity_event_repo,
            pool_repo,
            pool_metric_repo,
            rpc_client,
            swap_event_repo,
        }
    }

    /// Handle a transaction signature received from the WebSocket.
    pub(crate) async fn index_transaction(
        &self,
        protocol: Protocol,
        signature: Signature,
    ) -> anyhow::Result<()> {
        info!(%signature, protocol = %protocol.as_str(), "received signature");
        let tx = self
            .fetch_transaction(signature)
            .await?
            .ok_or_else(|| anyhow::anyhow!("transaction not found: {signature}"))?;

        let indexer = protocol_indexer(&protocol);




        // DEBUG temporaire — compter ce que le détecteur voit
        let is_swap = indexer.is_swap(&tx);
        let is_add = indexer.is_add_liquidity(&tx);
        let is_remove = indexer.is_remove_liquidity(&tx);
        tracing::info!(
            %signature,
            is_swap, is_add, is_remove,
            "detector results"
        );



        

        if indexer.is_swap(&tx) {
            let swap = indexer.parse_swap(&tx)?;
            info!(
                signature = %swap.signature,
                amount_in = swap.amount_in,
                amount_out = swap.amount_out,
                "swap parsed"
            );
            self.upsert_pool_from_swap(&swap).await?;
            self.persist_swap(&swap).await?;
            self.persist_metrics(&swap).await?;
        } else if indexer.is_add_liquidity(&tx) {
            let event = indexer.parse_add_liquidity(&tx)?;
            info!(
                signature = %event.signature,
                amount_a = event.amount_a,
                amount_b = event.amount_b,
                "add liquidity parsed"
            );
            self.upsert_pool_from_liquidity(&event).await?;
            self.persist_liquidity_event(&event).await?;
        } else if indexer.is_remove_liquidity(&tx) {
            let event = indexer.parse_remove_liquidity(&tx)?;
            info!(
                signature = %event.signature,
                amount_a = event.amount_a,
                amount_b = event.amount_b,
                "remove liquidity parsed"
            );
            self.upsert_pool_from_liquidity(&event).await?;

            self.persist_liquidity_event(&event).await?;
        } else {
            debug!("skipping unrecognised transaction: {signature}");
        }

        Ok(())
    }

    async fn persist_swap(&self, swap: &SwapEvent) -> anyhow::Result<()> {
        self.swap_event_repo.insert(swap).await?;
        Ok(())
    }

    async fn persist_metrics(&self, swap: &SwapEvent) -> anyhow::Result<()> {
        let metric = self.compute_metrics(swap)?;
        self.pool_metric_repo.insert(&metric).await?;
        Ok(())
    }

    async fn persist_liquidity_event(&self, event: &LiquidityEvent) -> anyhow::Result<()> {
        self.liquidity_event_repo.insert(event).await?;
        Ok(())
    }

    async fn upsert_pool_from_swap(&self, swap: &SwapEvent) -> anyhow::Result<()> {
        let now = Utc::now();
        let pool = Pool {
            pool_address: swap.pool_address,
            protocol: swap.protocol,
            token_a_mint: swap.token_a_mint,
            token_b_mint: swap.token_b_mint,
            first_seen_at: now,
            last_seen_at: now,
        };
        self.pool_repo.upsert(&pool).await?;
        Ok(())
    }

    async fn upsert_pool_from_liquidity(&self, event: &LiquidityEvent) -> anyhow::Result<()> {
        let now = Utc::now();
        let pool = Pool {
            pool_address: event.pool_address,
            protocol: event.protocol,
            token_a_mint: event.token_a_mint,
            token_b_mint: event.token_b_mint,
            first_seen_at: now,
            last_seen_at: now,
        };
        self.pool_repo.upsert(&pool).await?;
        Ok(())
    }

    /// Fetch a confirmed transaction by signature from the RPC.
    async fn fetch_transaction(
        &self,
        signature: Signature,
    ) -> anyhow::Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        let strategy = FixedInterval::from_millis(500).take(5);

        let result = Retry::spawn(strategy, || async {
            self.rpc_client
                .get_transaction_with_config(&signature, config)
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
        let reserve_a = swap.reserve_in_after as u128;
        let reserve_b = swap.reserve_out_after as u128;
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
            reserve_a: swap.reserve_in_after,
            reserve_b: swap.reserve_out_after,
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

fn protocol_indexer(protocol: &Protocol) -> Arc<dyn PoolIndexer> {
    match protocol {
        Protocol::MeteoraDammV2 => Arc::new(MeteoraDammV2::new()),
        Protocol::MeteoraDammV1 => Arc::new(MeteoraDammV1::new()),
        Protocol::MeteoraDlmm => Arc::new(MeteoraDlmm::new()),
    }
}
