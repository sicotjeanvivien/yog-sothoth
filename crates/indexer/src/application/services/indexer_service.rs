use chrono::Utc;
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedTransactionWithStatusMeta,
    UiTransactionEncoding,
};
use std::sync::Arc;
use tokio_retry::{strategy::FixedInterval, Retry};
use tracing::{info, warn};
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

use crate::application::services::IndexerServiceMetrics;

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

        // Extract every program-specific instruction name present in the
        // transaction logs. Used solely for observability — the parsing
        // flow below is unchanged and remains based on is_swap /
        // is_add_liquidity / is_remove_liquidity detectors.
        let instructions = extract_program_instructions(&tx, &indexer.program_id().to_string());

        let mut any_handled = false;

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
            IndexerServiceMetrics::record_indexed(&protocol, "Swap");
            any_handled = true;
        }
        if indexer.is_add_liquidity(&tx) {
            let event = indexer.parse_add_liquidity(&tx)?;
            info!(
                signature = %event.signature,
                amount_a = event.amount_a,
                amount_b = event.amount_b,
                "add liquidity parsed"
            );
            self.upsert_pool_from_liquidity(&event).await?;
            self.persist_liquidity_event(&event).await?;
            IndexerServiceMetrics::record_indexed(&protocol, "AddLiquidity");
            any_handled = true;
        }
        if indexer.is_remove_liquidity(&tx) {
            let event = indexer.parse_remove_liquidity(&tx)?;
            info!(
                signature = %event.signature,
                amount_a = event.amount_a,
                amount_b = event.amount_b,
                "remove liquidity parsed"
            );
            self.upsert_pool_from_liquidity(&event).await?;
            self.persist_liquidity_event(&event).await?;
            IndexerServiceMetrics::record_indexed(&protocol, "RemoveLiquidity");
            any_handled = true;
        }

        // Emit a skipped metric for every instruction present in the
        // transaction that the detectors above did not match. This
        // catches cases like Swap2 colocated with a v1 Swap — the v1
        // Swap is indexed, the Swap2 would be silently lost without
        // this accounting.
        for name in &instructions {
            if !is_matched_instruction(name, any_handled, &indexer, &tx) {
                IndexerServiceMetrics::record_skipped(&protocol, name);
            }
        }

        if !any_handled {
            IndexerServiceMetrics::record_no_match(&protocol);
            if instructions.is_empty() {
                info!(%signature, "skipped tx — no program instruction found in logs");
            } else {
                info!(%signature, instructions = ?instructions, "skipped tx — no matching parser");
            }
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

/// Extract every `Program log: Instruction: <Name>` entry that appears
/// within a `Program {program_id} invoke` frame in the transaction logs.
///
/// Returns the instruction names in order of appearance. A transaction
/// that invokes the program three times returns three entries.
fn extract_program_instructions(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id: &str,
) -> Vec<String> {
    let Some(meta) = tx.transaction.meta.as_ref() else {
        return Vec::new();
    };
    let OptionSerializer::Some(logs) = &meta.log_messages else {
        return Vec::new();
    };

    let invoke_marker = format!("Program {program_id} invoke");
    let mut instructions = Vec::new();
    let mut in_program = false;

    for log in logs {
        if log.starts_with(&invoke_marker) {
            in_program = true;
            continue;
        }
        if !in_program {
            continue;
        }
        if let Some(name) = log.strip_prefix("Program log: Instruction: ") {
            instructions.push(name.to_string());
        } else if log.starts_with("Program ") && !log.starts_with("Program log:") {
            // Exiting the program frame (success / failed / return).
            in_program = false;
        }
    }

    instructions
}

/// Approximate check: does this instruction name correspond to something
/// one of the detectors actually matched?
///
/// Used only to decide whether to increment the "skipped" metric. Relies
/// on the naming convention of DAMM v2 instructions (`Swap`, `AddLiquidity`,
/// `RemoveLiquidity`). Imperfect — a colocated Swap + Swap2 will still
/// mark Swap as matched and Swap2 as skipped, which is the intent.
fn is_matched_instruction(
    name: &str,
    any_handled: bool,
    indexer: &Arc<dyn PoolIndexer>,
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> bool {
    if !any_handled {
        return false;
    }
    match name {
        "Swap" => indexer.is_swap(tx),
        "AddLiquidity" => indexer.is_add_liquidity(tx),
        "RemoveLiquidity" => indexer.is_remove_liquidity(tx),
        _ => false,
    }
}
