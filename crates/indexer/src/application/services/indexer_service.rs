use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::sync::Arc;
use std::time::Instant;
use tokio_retry::{Retry, strategy::FixedInterval};
use tracing::{debug, error, info, warn};
use yog_core::{
    domain::{
        ClaimPositionFeeEventRepository, ClaimRewardEventRepository, DomainEvent, LiquidityEvent,
        LiquidityEventRepository, Pool, PoolCurrentStateRepository, PoolCurrentStateUpsert,
        PoolRepository, Protocol, SwapEvent, SwapEventRepository,
    },
    protocols::{
        PoolIndexer,
        extraction::{ExtractionFailure, ExtractionOutcome, discriminator_hex},
        meteora::{MeteoraDammV1, MeteoraDammV2, MeteoraDlmm},
    },
};

use crate::application::services::{IndexerServiceMetrics, errors::FetchError};

/// Core pipeline — receives a signature, fetches the full transaction,
/// dispatches to the appropriate protocol handler, persists every domain
/// event the handler extracts.
pub(crate) struct IndexerService {
    swap_event_repo: Arc<dyn SwapEventRepository>,
    liquidity_event_repo: Arc<dyn LiquidityEventRepository>,
    claim_position_fee_repo: Arc<dyn ClaimPositionFeeEventRepository>,
    claim_reward_repo: Arc<dyn ClaimRewardEventRepository>,
    pool_repo: Arc<dyn PoolRepository>,
    pool_current_state_repo: Arc<dyn PoolCurrentStateRepository>,
    rpc_client: Arc<RpcClient>,
}

impl IndexerService {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        swap_event_repo: Arc<dyn SwapEventRepository>,
        liquidity_event_repo: Arc<dyn LiquidityEventRepository>,
        claim_position_fee_repo: Arc<dyn ClaimPositionFeeEventRepository>,
        claim_reward_repo: Arc<dyn ClaimRewardEventRepository>,
        pool_repo: Arc<dyn PoolRepository>,
        pool_current_state_repo: Arc<dyn PoolCurrentStateRepository>,
        rpc_client: Arc<RpcClient>,
    ) -> Self {
        Self {
            swap_event_repo,
            liquidity_event_repo,
            claim_position_fee_repo,
            claim_reward_repo,
            pool_repo,
            pool_current_state_repo,
            rpc_client,
        }
    }

    /// Handle a transaction signature received from the WebSocket.
    ///
    /// Pipeline:
    ///   1. Fetch the full transaction from RPC.
    ///   2. Delegate event extraction to the protocol-specific handler.
    ///   3. Persist each extracted domain event independently — a failure
    ///      on one event never aborts the others (skip-and-log).
    ///   4. Surface unknown discriminators and extraction failures as
    ///      metrics + structured logs.
    pub(crate) async fn index_transaction(
        &self,
        protocol: Protocol,
        signature: Signature,
    ) -> anyhow::Result<()> {
        let mut guard = ExitGuard::new(protocol);

        info!(%signature, protocol = %protocol.as_str(), "received signature");

        let tx = match self.fetch_transaction(&protocol, signature).await {
            Ok(tx) => tx,
            Err(FetchError::NotFound) => {
                IndexerServiceMetrics::record_fetch_not_found(&protocol);
                guard.set("fetch_not_found");
                return Ok(());
            }
            Err(e) => {
                IndexerServiceMetrics::record_fetch_failure(&protocol, e.metric_label());
                guard.set("fetch_failure");
                return Err(e.into());
            }
        };

        let indexer = protocol_indexer(&protocol);

        // Extract all domain events the protocol handler can recognize.
        let outcome = match indexer.extract_events(&tx) {
            Ok(o) => o,
            Err(e) => {
                error!(%signature, error = %e, "extraction failed at transaction level");
                guard.set("extract_failure");
                return Err(e.into());
            }
        };

        self.report_diagnostics(&protocol, &signature, &outcome);

        if outcome.events.is_empty() {
            IndexerServiceMetrics::record_no_match(&protocol);
            debug!(%signature, "no recognized events in transaction");
            guard.set("no_events");
            return Ok(());
        }

        // Persist each event. Failures on one event don't abort the others.
        for event in &outcome.events {
            self.persist_event(&protocol, event).await;
        }

        guard.set("ok");
        Ok(())
    }

    /// Surface unknown discriminators and extraction failures via logs and
    /// metrics. Does not affect persistence.
    fn report_diagnostics(
        &self,
        protocol: &Protocol,
        signature: &Signature,
        outcome: &ExtractionOutcome,
    ) {
        for unknown in &outcome.unknown {
            let hex = discriminator_hex(&unknown.discriminator);
            debug!(
                %signature,
                protocol = %protocol.as_str(),
                discriminator = %hex,
                "unknown anchor event"
            );
            IndexerServiceMetrics::record_unknown_event(protocol, &hex);
        }

        for failure in &outcome.failures {
            let kind = failure_kind(failure);
            warn!(
                %signature,
                protocol = %protocol.as_str(),
                kind,
                error = %failure,
                "extraction failure"
            );
            IndexerServiceMetrics::record_extraction_failure(protocol, kind);
        }
    }

    /// Persist a single domain event, including its associated pool upsert
    /// or last-seen touch. Errors are logged and metrics are emitted, but
    /// they don't propagate — the caller continues with the next event.
    async fn persist_event(&self, protocol: &Protocol, event: &DomainEvent) {
        let kind = event.kind();
        let start = Instant::now();

        let result = match event {
            DomainEvent::Swap(e) => {
                if let Err(err) = self
                    .upsert_pool_full(
                        protocol,
                        e.pool_address,
                        e.protocol,
                        e.token_a_mint,
                        e.token_b_mint,
                    )
                    .await
                {
                    warn!(error = %err, kind, "pool upsert failed");
                }
                let insert_result = self
                    .swap_event_repo
                    .insert(e)
                    .await
                    .map_err(anyhow::Error::new);
                // Refresh the per-pool projection only if the event actually
                // landed in the append-only log — keeps current_state honest.
                if insert_result.is_ok() {
                    self.update_pool_current_state_from_swap(protocol, e).await;
                }
                insert_result
            }
            DomainEvent::Liquidity(e) => {
                if let Err(err) = self
                    .upsert_pool_full(
                        protocol,
                        e.pool_address,
                        e.protocol,
                        e.token_a_mint,
                        e.token_b_mint,
                    )
                    .await
                {
                    warn!(error = %err, kind, "pool upsert failed");
                }
                let insert_result = self
                    .liquidity_event_repo
                    .insert(e)
                    .await
                    .map_err(anyhow::Error::new);
                if insert_result.is_ok() {
                    self.update_pool_current_state_from_liquidity(protocol, e)
                        .await;
                }
                insert_result
            }
            DomainEvent::ClaimPositionFee(e) => {
                self.touch_pool(protocol, &e.pool_address).await;
                self.claim_position_fee_repo
                    .insert(e)
                    .await
                    .map_err(anyhow::Error::new)
            }
            DomainEvent::ClaimReward(e) => {
                self.touch_pool(protocol, &e.pool_address).await;
                self.claim_reward_repo
                    .insert(e)
                    .await
                    .map_err(anyhow::Error::new)
            }
        };

        let elapsed = start.elapsed().as_secs_f64();
        IndexerServiceMetrics::record_persist_duration(protocol, kind, elapsed);

        match result {
            Ok(()) => {
                IndexerServiceMetrics::record_indexed(protocol, kind);
            }
            Err(err) => {
                error!(
                    protocol = %protocol.as_str(),
                    kind,
                    error = %err,
                    "persist event failed"
                );
                IndexerServiceMetrics::record_persist_failure(protocol, kind);
            }
        }
    }

    /// Upsert the pool with full information (mints known).
    /// Used by Swap and Liquidity events.
    async fn upsert_pool_full(
        &self,
        protocol: &Protocol,
        pool_address: solana_pubkey::Pubkey,
        pool_protocol: Protocol,
        token_a_mint: solana_pubkey::Pubkey,
        token_b_mint: solana_pubkey::Pubkey,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now();
        let pool = Pool {
            pool_address,
            protocol: pool_protocol,
            token_a_mint,
            token_b_mint,
            first_seen_at: now,
            last_seen_at: now,
        };
        let start = Instant::now();
        self.pool_repo.upsert(&pool).await?;
        IndexerServiceMetrics::record_persist_duration(
            protocol,
            "pool_upsert",
            start.elapsed().as_secs_f64(),
        );
        Ok(())
    }

    /// Refresh `last_seen_at` for a pool. No-op if the pool is unknown
    /// (will be created when a Swap or Liquidity event arrives later).
    /// Used by ClaimPositionFee and ClaimReward events.
    async fn touch_pool(&self, protocol: &Protocol, pool_address: &solana_pubkey::Pubkey) {
        let start = Instant::now();
        match self.pool_repo.touch_last_seen(pool_address).await {
            Ok(()) => {
                IndexerServiceMetrics::record_persist_duration(
                    protocol,
                    "pool_touch",
                    start.elapsed().as_secs_f64(),
                );
            }
            Err(err) => {
                warn!(
                    protocol = %protocol.as_str(),
                    error = %err,
                    "pool touch_last_seen failed"
                );
            }
        }
    }

    /// Project a freshly-persisted swap event into `pool_current_state`.
    ///
    /// Best-effort: a failure here is logged but never aborts the caller.
    /// The SQL-side stale-write guard makes replays safe.
    async fn update_pool_current_state_from_swap(&self, protocol: &Protocol, event: &SwapEvent) {
        let upsert = PoolCurrentStateUpsert::from_swap(
            event.pool_address.to_string(),
            event.protocol.as_str().to_string(),
            event.timestamp,
            event.signature.to_string(),
            event.reserve_a_after,
            event.reserve_b_after,
            event.next_sqrt_price,
        );
        self.apply_pool_current_state_upsert(protocol, &upsert)
            .await;
    }

    /// Project a freshly-persisted liquidity event into `pool_current_state`.
    async fn update_pool_current_state_from_liquidity(
        &self,
        protocol: &Protocol,
        event: &LiquidityEvent,
    ) {
        let upsert = PoolCurrentStateUpsert::from_liquidity(
            event.pool_address.to_string(),
            event.protocol.as_str().to_string(),
            event.timestamp,
            event.signature.to_string(),
            event.liquidity_event_kind,
            event.reserve_a_after,
            event.reserve_b_after,
            event.liquidity_delta,
        );
        self.apply_pool_current_state_upsert(protocol, &upsert)
            .await;
    }

    /// Shared call site for the projection upsert. Records timing and
    /// classifies the outcome (`applied` vs `stale`) as a metric label so
    /// stale-write rates can be observed in Prometheus.
    async fn apply_pool_current_state_upsert(
        &self,
        protocol: &Protocol,
        upsert: &PoolCurrentStateUpsert,
    ) {
        let start = Instant::now();
        match self.pool_current_state_repo.upsert(upsert).await {
            Ok(applied) => {
                let label = if applied {
                    "pool_current_state_applied"
                } else {
                    "pool_current_state_stale"
                };
                IndexerServiceMetrics::record_persist_duration(
                    protocol,
                    label,
                    start.elapsed().as_secs_f64(),
                );
            }
            Err(err) => {
                warn!(
                    protocol = %protocol.as_str(),
                    error = %err,
                    "pool_current_state upsert failed"
                );
            }
        }
    }

    /// Fetch a confirmed transaction by signature from the RPC.
    ///
    /// Returns the typed `FetchError` so the caller can match on the
    /// failure mode (e.g. treat `NotFound` as a metric-only outcome
    /// while propagating other variants as real errors).
    async fn fetch_transaction(
        &self,
        protocol: &Protocol,
        signature: Signature,
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta, FetchError> {
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        let strategy = FixedInterval::from_millis(500).take(5);

        let start = Instant::now();
        let result = Retry::spawn(strategy, || async {
            self.rpc_client
                .get_transaction_with_config(&signature, config)
                .await
                .map_err(|e| e.to_string())
        })
        .await;
        IndexerServiceMetrics::record_fetch_duration(protocol, start.elapsed().as_secs_f64());

        result.map_err(FetchError::from_rpc_string)
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn protocol_indexer(protocol: &Protocol) -> Arc<dyn PoolIndexer> {
    match protocol {
        Protocol::MeteoraDammV2 => Arc::new(MeteoraDammV2::new()),
        Protocol::MeteoraDammV1 => Arc::new(MeteoraDammV1::new()),
        Protocol::MeteoraDlmm => Arc::new(MeteoraDlmm::new()),
    }
}

/// Stable label for an `ExtractionFailure` variant — used as a metric label
/// so we can distinguish anchor / borsh / translation failures.
fn failure_kind(f: &ExtractionFailure) -> &'static str {
    match f {
        ExtractionFailure::AnchorDecode(_) => "anchor_decode",
        ExtractionFailure::Borsh { .. } => "borsh",
        ExtractionFailure::Translation { .. } => "translation",
    }
}

// ---------------------------------------------------------------------------
// ExitGuard
// ---------------------------------------------------------------------------

/// RAII guard that records the outcome and duration of `index_transaction`.
struct ExitGuard {
    protocol: Protocol,
    outcome: Option<&'static str>,
    start: Instant,
}

impl ExitGuard {
    fn new(protocol: Protocol) -> Self {
        IndexerServiceMetrics::record_entered(&protocol);
        Self {
            protocol,
            outcome: None,
            start: Instant::now(),
        }
    }

    fn set(&mut self, outcome: &'static str) {
        self.outcome = Some(outcome);
    }
}

impl Drop for ExitGuard {
    fn drop(&mut self) {
        let outcome = self.outcome.unwrap_or("unknown_exit");
        IndexerServiceMetrics::record_exited(&self.protocol, outcome);
        IndexerServiceMetrics::record_index_tx_duration(
            &self.protocol,
            outcome,
            self.start.elapsed().as_secs_f64(),
        );
    }
}
