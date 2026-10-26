//! Meteora DAMM v2 event persistor: dispatches each event to its persistence recipe; pool
//! upserts and the current-state projection go through [`PoolMaintenance`].

use std::sync::Arc;
use std::time::Instant;

use solana_pubkey::Pubkey;
use tracing::{debug, error, warn};
use yog_core::application::{
    MeteoraDammV2FeeParametersRejection, decode_meteora_damm_v2_pool_fees,
    decode_meteora_damm_v2_pool_fees_update,
};
use yog_core::domain::{
    InsertOutcome, MeteoraDammV2ClaimPositionFeeEvent,
    MeteoraDammV2ClaimPositionFeeEventRepository, MeteoraDammV2ClaimProtocolFeeEvent,
    MeteoraDammV2ClaimProtocolFeeEventRepository, MeteoraDammV2ClaimRewardEvent,
    MeteoraDammV2ClaimRewardEventRepository, MeteoraDammV2ClosePositionEvent,
    MeteoraDammV2ClosePositionEventRepository, MeteoraDammV2CreatePositionEvent,
    MeteoraDammV2CreatePositionEventRepository, MeteoraDammV2Event, MeteoraDammV2FundRewardEvent,
    MeteoraDammV2FundRewardEventRepository, MeteoraDammV2InitializePoolEvent,
    MeteoraDammV2InitializePoolEventRepository, MeteoraDammV2InitializeRewardEvent,
    MeteoraDammV2InitializeRewardEventRepository, MeteoraDammV2LiquidityEvent,
    MeteoraDammV2LiquidityEventRepository, MeteoraDammV2LockPositionEvent,
    MeteoraDammV2LockPositionEventRepository, MeteoraDammV2PermanentLockPositionEvent,
    MeteoraDammV2PermanentLockPositionEventRepository, MeteoraDammV2SetPoolStatusEvent,
    MeteoraDammV2SetPoolStatusEventRepository, MeteoraDammV2SplitPositionEvent,
    MeteoraDammV2SplitPositionEventRepository, MeteoraDammV2SwapEvent,
    MeteoraDammV2SwapEventRepository, MeteoraDammV2UpdatePoolFeesEvent,
    MeteoraDammV2UpdatePoolFeesEventRepository, MeteoraDammV2UpdateRewardDurationEvent,
    MeteoraDammV2UpdateRewardDurationEventRepository, MeteoraDammV2UpdateRewardFunderEvent,
    MeteoraDammV2UpdateRewardFunderEventRepository, MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    MeteoraDammV2WithdrawIneligibleRewardEvent,
    MeteoraDammV2WithdrawIneligibleRewardEventRepository, Protocol,
};

use crate::application::services::{EventPersistorMetrics, PoolMaintenance};

/// The per-event-kind DAMM v2 repositories, bundled into one named-field argument.
pub(crate) struct DammV2Repos {
    pub swap_event: Arc<dyn MeteoraDammV2SwapEventRepository>,
    pub liquidity_event: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
    pub claim_position_fee: Arc<dyn MeteoraDammV2ClaimPositionFeeEventRepository>,
    pub claim_protocol_fee: Arc<dyn MeteoraDammV2ClaimProtocolFeeEventRepository>,
    pub claim_reward: Arc<dyn MeteoraDammV2ClaimRewardEventRepository>,
    pub initialize_reward: Arc<dyn MeteoraDammV2InitializeRewardEventRepository>,
    pub fund_reward: Arc<dyn MeteoraDammV2FundRewardEventRepository>,
    pub withdraw_ineligible_reward: Arc<dyn MeteoraDammV2WithdrawIneligibleRewardEventRepository>,
    pub update_reward_duration: Arc<dyn MeteoraDammV2UpdateRewardDurationEventRepository>,
    pub update_reward_funder: Arc<dyn MeteoraDammV2UpdateRewardFunderEventRepository>,
    pub withdraw_dead_liquidity_reward:
        Arc<dyn MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository>,
    pub create_position: Arc<dyn MeteoraDammV2CreatePositionEventRepository>,
    pub close_position: Arc<dyn MeteoraDammV2ClosePositionEventRepository>,
    pub lock_position: Arc<dyn MeteoraDammV2LockPositionEventRepository>,
    pub permanent_lock_position: Arc<dyn MeteoraDammV2PermanentLockPositionEventRepository>,
    pub initialize_pool: Arc<dyn MeteoraDammV2InitializePoolEventRepository>,
    pub set_pool_status: Arc<dyn MeteoraDammV2SetPoolStatusEventRepository>,
    pub split_position: Arc<dyn MeteoraDammV2SplitPositionEventRepository>,
    pub update_pool_fees: Arc<dyn MeteoraDammV2UpdatePoolFeesEventRepository>,
    // ⚠️ No pool-properties repository: yog-context writes the satellite from the account, and
    // this crate raises `pools.needs_refresh` instead.
}

pub(crate) struct MeteoraDammV2EventPersistor {
    repos: DammV2Repos,
    pool_maintenance: Arc<PoolMaintenance>,
}

impl MeteoraDammV2EventPersistor {
    const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

    pub(crate) fn new(repos: DammV2Repos, pool_maintenance: Arc<PoolMaintenance>) -> Self {
        Self {
            repos,
            pool_maintenance,
        }
    }

    /// Apply a Meteora DAMM v2 event to its persistence targets; a failure is logged and counted,
    /// never propagated.
    pub(crate) async fn persist(&self, event: &MeteoraDammV2Event) {
        let kind = event.kind();
        let start = Instant::now();

        let result = match event {
            MeteoraDammV2Event::Swap(e) => self.persist_swap(e, kind).await,
            MeteoraDammV2Event::Liquidity(e) => self.persist_liquidity(e, kind).await,
            MeteoraDammV2Event::ClaimPositionFee(e) => self.persist_claim_position_fee(e).await,
            MeteoraDammV2Event::ClaimProtocolFee(e) => self.persist_claim_protocol_fee(e).await,
            MeteoraDammV2Event::ClaimReward(e) => self.persist_claim_reward(e).await,
            MeteoraDammV2Event::InitializeReward(e) => self.persist_initialize_reward(e).await,
            MeteoraDammV2Event::FundReward(e) => self.persist_fund_reward(e).await,
            MeteoraDammV2Event::WithdrawIneligibleReward(e) => {
                self.persist_withdraw_ineligible_reward(e).await
            }
            MeteoraDammV2Event::UpdateRewardDuration(e) => {
                self.persist_update_reward_duration(e).await
            }
            MeteoraDammV2Event::UpdateRewardFunder(e) => self.persist_update_reward_funder(e).await,
            MeteoraDammV2Event::WithdrawDeadLiquidityReward(e) => {
                self.persist_withdraw_dead_liquidity_reward(e).await
            }
            MeteoraDammV2Event::CreatePosition(e) => self.persist_create_position(e).await,
            MeteoraDammV2Event::ClosePosition(e) => self.persist_close_position(e).await,
            MeteoraDammV2Event::LockPosition(e) => self.persist_lock_position(e).await,
            MeteoraDammV2Event::PermanentLockPosition(e) => {
                self.persist_permanent_lock_position(e).await
            }
            MeteoraDammV2Event::InitializePool(e) => self.persist_initialize_pool(e, kind).await,
            MeteoraDammV2Event::SetPoolStatus(e) => self.persist_set_pool_status(e).await,
            MeteoraDammV2Event::SplitPosition(e) => self.persist_split_position(e).await,
            MeteoraDammV2Event::UpdatePoolFees(e) => self.persist_update_pool_fees(e, kind).await,
        };

        let elapsed = start.elapsed().as_secs_f64();
        EventPersistorMetrics::record_persist_duration(&Self::PROTOCOL, kind, elapsed);

        match result {
            Ok(outcome) => {
                EventPersistorMetrics::record_indexed(&Self::PROTOCOL, kind);
                if outcome.is_skipped() {
                    // The counter is the signal: two events sharing a unique key on a live
                    // stream. `debug` here, or a replay would warn once per event backfilled.
                    debug!(
                        protocol = %Self::PROTOCOL.as_str(),
                        kind,
                        "event insert skipped: a row with the same key already existed"
                    );
                    EventPersistorMetrics::record_insert_skipped(&Self::PROTOCOL, kind);
                }
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

    async fn persist_swap(
        &self,
        event: &MeteoraDammV2SwapEvent,
        kind: &'static str,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address, kind)
            .await;
        let insert_result = self
            .repos
            .swap_event
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

    async fn persist_liquidity(
        &self,
        event: &MeteoraDammV2LiquidityEvent,
        kind: &'static str,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address, kind)
            .await;
        let insert_result = self
            .repos
            .liquidity_event
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
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .claim_position_fee
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_claim_protocol_fee(
        &self,
        event: &MeteoraDammV2ClaimProtocolFeeEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .claim_protocol_fee
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_claim_reward(
        &self,
        event: &MeteoraDammV2ClaimRewardEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .claim_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// The reward mint is a third token, not a pool side: touch + insert.
    async fn persist_initialize_reward(
        &self,
        event: &MeteoraDammV2InitializeRewardEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .initialize_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Funding moves the reward token, not a pool side: touch + insert.
    async fn persist_fund_reward(
        &self,
        event: &MeteoraDammV2FundRewardEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .fund_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Moves the reward token out of its vault, not a pool side: touch + insert.
    async fn persist_withdraw_ineligible_reward(
        &self,
        event: &MeteoraDammV2WithdrawIneligibleRewardEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .withdraw_ineligible_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Re-pacing a farm slot changes no pool reserve — touch + insert.
    async fn persist_update_reward_duration(
        &self,
        event: &MeteoraDammV2UpdateRewardDurationEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .update_reward_duration
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Moving the funding right moves no tokens at all — touch + insert.
    async fn persist_update_reward_funder(
        &self,
        event: &MeteoraDammV2UpdateRewardFunderEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .update_reward_funder
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Moves the reward token out of its vault, not a pool side: touch + insert.
    async fn persist_withdraw_dead_liquidity_reward(
        &self,
        event: &MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .withdraw_dead_liquidity_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// No mints or reserves: touch + insert, no registry upsert and no projection.
    async fn persist_create_position(
        &self,
        event: &MeteoraDammV2CreatePositionEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .create_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Same recipe as create-position: touch + insert.
    async fn persist_close_position(
        &self,
        event: &MeteoraDammV2ClosePositionEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .close_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Vesting params but no pool reserves: touch + insert.
    async fn persist_lock_position(
        &self,
        event: &MeteoraDammV2LockPositionEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .lock_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_permanent_lock_position(
        &self,
        event: &MeteoraDammV2PermanentLockPositionEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .permanent_lock_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Pool genesis registers the pool (`discover_pool`); no projection, there is no reserve
    /// trajectory yet.
    /// ⚠️ Its mints stay on the event's table: `pools` takes them from yog-context alone.
    async fn persist_initialize_pool(
        &self,
        event: &MeteoraDammV2InitializePoolEvent,
        kind: &'static str,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address, kind)
            .await;
        // Fee columns are yog-context's, read from the account; `discover_pool` leaves them NULL,
        // which queues the pool for enrichment. The blob is stored verbatim by the insert.
        if let Err(reason) = decode_meteora_damm_v2_pool_fees(&event.pool_fees_raw) {
            Self::fees_undecodable(kind, &event.pool_address, &reason);
        }
        self.repos
            .initialize_pool
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// A split moves contents between two positions, nothing in or out of the pool: touch + insert.
    async fn persist_split_position(
        &self,
        event: &MeteoraDammV2SplitPositionEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .split_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_set_pool_status(
        &self,
        event: &MeteoraDammV2SetPoolStatusEvent,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .set_pool_status
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Fee parameters the decoder cannot read: logged and counted, the event stored raw anyway.
    fn fees_undecodable(
        kind: &'static str,
        pool: &Pubkey,
        reason: &MeteoraDammV2FeeParametersRejection,
    ) {
        warn!(%pool, kind, %reason, "fee parameters undecodable, stored raw");
        EventPersistorMetrics::record_fee_parameters_undecodable(
            &Self::PROTOCOL,
            kind,
            reason.as_str(),
        );
    }

    async fn persist_update_pool_fees(
        &self,
        event: &MeteoraDammV2UpdatePoolFeesEvent,
        kind: &'static str,
    ) -> anyhow::Result<InsertOutcome> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        // ⚠️ Flag the pool rather than write what the update changed: yog-context rewrites every
        // property from the account, one writer per column.
        if let Err(err) = self
            .pool_maintenance
            .mark_needs_refresh(Self::PROTOCOL, &event.pool_address)
            .await
        {
            warn!(error = %err, kind, "mark_needs_refresh failed");
        }
        if let Err(reason) = decode_meteora_damm_v2_pool_fees_update(&event.params_raw) {
            Self::fees_undecodable(kind, &event.pool_address, &reason);
        }
        self.repos
            .update_pool_fees
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }
}

#[cfg(test)]
#[path = "event_persistor_tests.rs"]
mod tests;
