//! Meteora DAMM v2 event persistor.
//!
//! Owns the four DAMM v2-specific repositories and dispatches each
//! event kind to its persistence recipe. Pool registry upserts and
//! pool_current_state projection are delegated to the shared
//! [`PoolMaintenance`] helper.

use std::sync::Arc;
use std::time::Instant;
use tracing::{error, warn};
use yog_core::domain::{
    MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
    MeteoraDammV2ClaimRewardEvent, MeteoraDammV2ClaimRewardEventRepository,
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2ClosePositionEventRepository,
    MeteoraDammV2CreatePositionEvent, MeteoraDammV2CreatePositionEventRepository,
    MeteoraDammV2Event, MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventRepository,
    MeteoraDammV2SwapEvent, MeteoraDammV2SwapEventRepository, Protocol,
};

use crate::application::services::{EventPersistorMetrics, PoolMaintenance};

pub(crate) struct MeteoraDammV2EventPersistor {
    swap_event_repo: Arc<dyn MeteoraDammV2SwapEventRepository>,
    liquidity_event_repo: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
    claim_position_fee_repo: Arc<dyn MeteoraDammV2ClaimPositionFeeEventRepository>,
    claim_reward_repo: Arc<dyn MeteoraDammV2ClaimRewardEventRepository>,
    create_position_repo: Arc<dyn MeteoraDammV2CreatePositionEventRepository>,
    close_position_repo: Arc<dyn MeteoraDammV2ClosePositionEventRepository>,
    pool_maintenance: Arc<PoolMaintenance>,
}

impl MeteoraDammV2EventPersistor {
    const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

    pub(crate) fn new(
        swap_event_repo: Arc<dyn MeteoraDammV2SwapEventRepository>,
        liquidity_event_repo: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
        claim_position_fee_repo: Arc<dyn MeteoraDammV2ClaimPositionFeeEventRepository>,
        claim_reward_repo: Arc<dyn MeteoraDammV2ClaimRewardEventRepository>,
        create_position_repo: Arc<dyn MeteoraDammV2CreatePositionEventRepository>,
        close_position_repo: Arc<dyn MeteoraDammV2ClosePositionEventRepository>,
        pool_maintenance: Arc<PoolMaintenance>,
    ) -> Self {
        Self {
            swap_event_repo,
            liquidity_event_repo,
            claim_position_fee_repo,
            claim_reward_repo,
            create_position_repo,
            close_position_repo,
            pool_maintenance,
        }
    }

    /// Apply a Meteora DAMM v2 event to its persistence targets.
    ///
    /// Per-event failures are logged and counted, never propagated —
    /// the caller continues with the next event.
    pub(crate) async fn persist(&self, event: &MeteoraDammV2Event) {
        let kind = event.kind();
        let start = Instant::now();

        let result = match event {
            MeteoraDammV2Event::Swap(e) => self.persist_swap(e).await,
            MeteoraDammV2Event::Liquidity(e) => self.persist_liquidity(e).await,
            MeteoraDammV2Event::ClaimPositionFee(e) => self.persist_claim_position_fee(e).await,
            MeteoraDammV2Event::ClaimReward(e) => self.persist_claim_reward(e).await,
            MeteoraDammV2Event::CreatePosition(e) => self.persist_create_position(e).await,
            MeteoraDammV2Event::ClosePosition(e) => self.persist_close_position(e).await,
        };

        let elapsed = start.elapsed().as_secs_f64();
        EventPersistorMetrics::record_persist_duration(&Self::PROTOCOL, kind, elapsed);

        match result {
            Ok(()) => {
                EventPersistorMetrics::record_indexed(&Self::PROTOCOL, kind);
            }
            Err(err) => {
                error!(
                    protocol = %Self::PROTOCOL.as_str(),
                    kind,
                    error = %err,
                    "persist event failed"
                );
                EventPersistorMetrics::record_persist_failure(&Self::PROTOCOL, kind);
            }
        }
    }

    async fn persist_swap(&self, event: &MeteoraDammV2SwapEvent) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .upsert_pool_full(
                Self::PROTOCOL,
                event.pool_address,
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
        if insert_result.is_ok() {
            self.pool_maintenance
                .update_pool_current_state_from_swap(Self::PROTOCOL, event)
                .await;
        }
        insert_result
    }

    async fn persist_liquidity(&self, event: &MeteoraDammV2LiquidityEvent) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .upsert_pool_full(
                Self::PROTOCOL,
                event.pool_address,
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
            self.pool_maintenance
                .update_pool_current_state_from_liquidity(Self::PROTOCOL, event)
                .await;
        }
        insert_result
    }

    async fn persist_claim_position_fee(
        &self,
        event: &MeteoraDammV2ClaimPositionFeeEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.claim_position_fee_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_claim_reward(
        &self,
        event: &MeteoraDammV2ClaimRewardEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.claim_reward_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// A create-position event carries no mints or reserves, so it neither
    /// upserts the pool registry nor updates the current-state projection —
    /// it only refreshes the pool's last-seen marker and records the event.
    async fn persist_create_position(
        &self,
        event: &MeteoraDammV2CreatePositionEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.create_position_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Same recipe as create-position: no mints/reserves, so just refresh the
    /// pool's last-seen marker and record the event.
    async fn persist_close_position(
        &self,
        event: &MeteoraDammV2ClosePositionEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.close_position_repo
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }
}
