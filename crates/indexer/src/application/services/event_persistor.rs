//! Apply a domain event to its persistence targets — the append-only
//! event log, the pool registry, and the per-pool current-state projection.
//!
//! Per-event failures are logged and counted, never propagated.
//! Pool-side operations (`upsert_pool_full`, `touch_pool`) and the
//! `pool_current_state` projection are best-effort: a failure on the
//! pool side never aborts the event insert, and a failure on the
//! projection never aborts the caller.

use std::sync::Arc;
use std::time::Instant;
use tracing::{error, warn};
use yog_core::domain::{
    DomainEvent, MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
    MeteoraDammV2ClaimRewardEvent, MeteoraDammV2ClaimRewardEventRepository,
    MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventRepository, MeteoraDammV2SwapEvent,
    MeteoraDammV2SwapEventRepository, Pool, PoolCurrentStateRepository, PoolCurrentStateUpsert,
    PoolRepository, Protocol,
};

use crate::application::services::EventPersistorMetrics;

pub(crate) struct EventPersistor {
    swap_event_repo: Arc<dyn MeteoraDammV2SwapEventRepository>,
    liquidity_event_repo: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
    claim_position_fee_repo: Arc<dyn MeteoraDammV2ClaimPositionFeeEventRepository>,
    claim_reward_repo: Arc<dyn MeteoraDammV2ClaimRewardEventRepository>,
    pool_repo: Arc<dyn PoolRepository>,
    pool_current_state_repo: Arc<dyn PoolCurrentStateRepository>,
}

impl EventPersistor {
    pub(crate) fn new(
        swap_event_repo: Arc<dyn MeteoraDammV2SwapEventRepository>,
        liquidity_event_repo: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
        claim_position_fee_repo: Arc<dyn MeteoraDammV2ClaimPositionFeeEventRepository>,
        claim_reward_repo: Arc<dyn MeteoraDammV2ClaimRewardEventRepository>,
        pool_repo: Arc<dyn PoolRepository>,
        pool_current_state_repo: Arc<dyn PoolCurrentStateRepository>,
    ) -> Self {
        Self {
            swap_event_repo,
            liquidity_event_repo,
            claim_position_fee_repo,
            claim_reward_repo,
            pool_repo,
            pool_current_state_repo,
        }
    }

    /// Apply a domain event to its persistence targets.
    ///
    /// Per-event failures are logged and counted, never propagated —
    /// the caller continues with the next event.
    pub(crate) async fn persist(&self, protocol: &Protocol, event: &DomainEvent) {
        let kind = event.kind();
        let start = Instant::now();

        let result = match event {
            DomainEvent::Swap(e) => self.persist_swap(protocol, e).await,
            DomainEvent::Liquidity(e) => self.persist_liquidity(protocol, e).await,
            DomainEvent::ClaimPositionFee(e) => self.persist_claim_position_fee(protocol, e).await,
            DomainEvent::ClaimReward(e) => self.persist_claim_reward(protocol, e).await,
        };

        let elapsed = start.elapsed().as_secs_f64();
        EventPersistorMetrics::record_persist_duration(protocol, kind, elapsed);

        match result {
            Ok(()) => {
                EventPersistorMetrics::record_indexed(protocol, kind);
            }
            Err(err) => {
                error!(
                    protocol = %protocol.as_str(),
                    kind,
                    error = %err,
                    "persist event failed"
                );
                EventPersistorMetrics::record_persist_failure(protocol, kind);
            }
        }
    }

    /// Persist a swap event: upsert the pool, insert the event, refresh the
    /// per-pool projection if the event landed.
    async fn persist_swap(
        &self,
        protocol: &Protocol,
        event: &MeteoraDammV2SwapEvent,
    ) -> anyhow::Result<()> {
        if let Err(err) = self
            .upsert_pool_full(
                protocol,
                event.pool_address,
                event.protocol,
                event.token_a_mint,
                event.token_b_mint,
            )
            .await
        {
            warn!(error = %err, kind = "swap", "pool upsert failed");
        }
        let insert_result = self
            .swap_event_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new);
        // Refresh the per-pool projection only if the event actually
        // landed in the append-only log — keeps current_state honest.
        if insert_result.is_ok() {
            self.update_pool_current_state_from_swap(protocol, event)
                .await;
        }
        insert_result
    }

    /// Persist a liquidity event: same shape as `persist_swap`.
    async fn persist_liquidity(
        &self,
        protocol: &Protocol,
        event: &LiquidityEvent,
    ) -> anyhow::Result<()> {
        if let Err(err) = self
            .upsert_pool_full(
                protocol,
                event.pool_address,
                event.protocol,
                event.token_a_mint,
                event.token_b_mint,
            )
            .await
        {
            warn!(error = %err, kind = "liquidity", "pool upsert failed");
        }
        let insert_result = self
            .liquidity_event_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new);
        if insert_result.is_ok() {
            self.update_pool_current_state_from_liquidity(protocol, event)
                .await;
        }
        insert_result
    }

    /// Persist a claim-position-fee event: refresh the pool's last_seen_at
    /// and insert the event.
    async fn persist_claim_position_fee(
        &self,
        protocol: &Protocol,
        event: &ClaimPositionFeeEvent,
    ) -> anyhow::Result<()> {
        self.touch_pool(protocol, &event.pool_address).await;
        self.claim_position_fee_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Persist a claim-reward event: refresh the pool's last_seen_at and
    /// insert the event.
    async fn persist_claim_reward(
        &self,
        protocol: &Protocol,
        event: &ClaimRewardEvent,
    ) -> anyhow::Result<()> {
        self.touch_pool(protocol, &event.pool_address).await;
        self.claim_reward_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
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
        EventPersistorMetrics::record_persist_duration(
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
                EventPersistorMetrics::record_persist_duration(
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
    async fn update_pool_current_state_from_swap(&self, protocol: &Protocol, event: &SwapEvent) {
        let upsert = PoolCurrentStateUpsert::from_swap(
            event.pool_address,
            event.protocol,
            event.timestamp,
            event.signature,
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
            event.pool_address,
            event.protocol,
            event.timestamp,
            event.signature,
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
                EventPersistorMetrics::record_persist_duration(
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
}
