//! Meteora DAMM v2 event persistor.
//!
//! Owns the per-event-kind DAMM v2 repositories and dispatches each
//! event to its persistence recipe. Pool registry upserts and
//! pool_current_state projection are delegated to the shared
//! [`PoolMaintenance`] helper.

use std::sync::Arc;
use std::time::Instant;
use tracing::{error, warn};
use yog_core::domain::{
    MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
    MeteoraDammV2ClaimProtocolFeeEvent, MeteoraDammV2ClaimProtocolFeeEventRepository,
    MeteoraDammV2ClaimRewardEvent, MeteoraDammV2ClaimRewardEventRepository,
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2ClosePositionEventRepository,
    MeteoraDammV2CreatePositionEvent, MeteoraDammV2CreatePositionEventRepository,
    MeteoraDammV2Event, MeteoraDammV2FundRewardEvent, MeteoraDammV2FundRewardEventRepository,
    MeteoraDammV2InitializePoolEvent, MeteoraDammV2InitializePoolEventRepository,
    MeteoraDammV2InitializeRewardEvent, MeteoraDammV2InitializeRewardEventRepository,
    MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventRepository,
    MeteoraDammV2LockPositionEvent, MeteoraDammV2LockPositionEventRepository,
    MeteoraDammV2PermanentLockPositionEvent, MeteoraDammV2PermanentLockPositionEventRepository,
    MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2SetPoolStatusEventRepository,
    MeteoraDammV2SwapEvent, MeteoraDammV2SwapEventRepository, MeteoraDammV2UpdatePoolFeesEvent,
    MeteoraDammV2UpdatePoolFeesEventRepository, MeteoraDammV2UpdateRewardDurationEvent,
    MeteoraDammV2UpdateRewardDurationEventRepository, MeteoraDammV2UpdateRewardFunderEvent,
    MeteoraDammV2UpdateRewardFunderEventRepository, MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    MeteoraDammV2WithdrawIneligibleRewardEvent,
    MeteoraDammV2WithdrawIneligibleRewardEventRepository, Protocol,
};

use crate::application::services::{EventPersistorMetrics, PoolMaintenance};

/// The per-event-kind DAMM v2 repositories, bundled so the persistor takes a
/// single named-field argument instead of one positional `Arc` per event kind
/// (which grows unbounded as new events are added).
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
    pub update_pool_fees: Arc<dyn MeteoraDammV2UpdatePoolFeesEventRepository>,
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
            MeteoraDammV2Event::InitializePool(e) => self.persist_initialize_pool(e).await,
            MeteoraDammV2Event::SetPoolStatus(e) => self.persist_set_pool_status(e).await,
            MeteoraDammV2Event::UpdatePoolFees(e) => self.persist_update_pool_fees(e).await,
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
            .discover_pool(Self::PROTOCOL, event.pool_address)
            .await
        {
            warn!(error = %err, kind = "swap", "pool upsert failed");
        }
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

    async fn persist_liquidity(&self, event: &MeteoraDammV2LiquidityEvent) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address)
            .await
        {
            warn!(error = %err, kind = "liquidity", "pool upsert failed");
        }
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
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .claim_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Opening a farm reward slot carries no mints or reserves for the *pool*
    /// (the reward mint is a third token, not a pool side) — so it only
    /// refreshes the pool's last-seen marker and records the event.
    async fn persist_initialize_reward(
        &self,
        event: &MeteoraDammV2InitializeRewardEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .initialize_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Funding a farm moves the reward token, not a pool side — no reserve or
    /// mint update for the pool itself. Touch + insert, like the other
    /// non-AMM-mutating events.
    async fn persist_fund_reward(
        &self,
        event: &MeteoraDammV2FundRewardEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .fund_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Returning unearnable rewards moves the reward token out of its vault,
    /// not a pool side — touch + insert, no reserve or projection update.
    async fn persist_withdraw_ineligible_reward(
        &self,
        event: &MeteoraDammV2WithdrawIneligibleRewardEvent,
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .update_reward_funder
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Reclaiming dead liquidity's reward share moves the reward token out of
    /// its vault, not a pool side — touch + insert.
    async fn persist_withdraw_dead_liquidity_reward(
        &self,
        event: &MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .withdraw_dead_liquidity_reward
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
        self.repos
            .create_position
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
        self.repos
            .close_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Lock carries vesting params but no pool reserves — same touch_pool +
    /// insert recipe as the other lifecycle events.
    async fn persist_lock_position(
        &self,
        event: &MeteoraDammV2LockPositionEvent,
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .permanent_lock_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Pool genesis carries both mints, so it registers the pool authoritatively
    /// (full upsert) rather than just touching last-seen. It does not feed the
    /// current-state projection — there is no price/reserve trajectory yet.
    ///
    /// The InitializePool event does carry the mints, but the `pools` registry
    /// gets them from a single source — yog-context decoding the pool account —
    /// to avoid a dual-write that could disagree on A/B ordering. So here we
    /// only register the pool; the mints land on the event's own table.
    async fn persist_initialize_pool(
        &self,
        event: &MeteoraDammV2InitializePoolEvent,
    ) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address)
            .await
        {
            warn!(error = %err, kind = "initialize_pool", "pool upsert failed");
        }
        // Decode the headline base fee from the genesis "voie C" blob and record
        // it as a pool property. Skip-and-log on an undecodable blob (unknown
        // fee mode): the pool keeps a NULL fee_bps rather than a wrong value.
        match yog_core::amm::damm_v2::decode_base_fee_bps(&event.pool_fees_raw) {
            Ok(fee_bps) => {
                self.pool_maintenance
                    .set_fee_bps(Self::PROTOCOL, &event.pool_address, fee_bps)
                    .await;
            }
            Err(err) => {
                warn!(error = %err, kind = "initialize_pool", "fee_bps decode failed");
            }
        }
        // Decode the fee *shape* (base-fee kind + dynamic-fee flag) from the
        // same genesis blob and record it. Independent of the fee tier above:
        // one may decode while the other fails. Skip-and-log keeps the columns
        // NULL rather than wrong.
        match yog_core::amm::damm_v2::decode_fee_config(&event.pool_fees_raw) {
            Ok(cfg) => {
                self.pool_maintenance
                    .set_fee_config(
                        Self::PROTOCOL,
                        &event.pool_address,
                        cfg.base_kind.as_str(),
                        cfg.has_dynamic_fee,
                    )
                    .await;
            }
            Err(err) => {
                warn!(error = %err, kind = "initialize_pool", "fee_config decode failed");
            }
        }
        self.repos
            .initialize_pool
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_set_pool_status(
        &self,
        event: &MeteoraDammV2SetPoolStatusEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .set_pool_status
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_update_pool_fees(
        &self,
        event: &MeteoraDammV2UpdatePoolFeesEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        // An operator fee change refreshes the pool's headline fee tier — but
        // only when it actually touched the base fee (Some). Skip-and-log on an
        // undecodable blob: keep the previous fee_bps rather than a wrong value.
        match yog_core::amm::damm_v2::decode_updated_base_fee_bps(&event.params_raw) {
            Ok(Some(fee_bps)) => {
                self.pool_maintenance
                    .set_fee_bps(Self::PROTOCOL, &event.pool_address, fee_bps)
                    .await;
            }
            Ok(None) => {}
            Err(err) => {
                warn!(error = %err, kind = "update_pool_fees", "fee_bps decode failed");
            }
        }
        self.repos
            .update_pool_fees
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }
}

// ---------------------------------------------------------------------------
// Tests — dispatch + recipe routing
// ---------------------------------------------------------------------------
//
// Verify that `persist()` routes each `MeteoraDammV2Event` variant to the
// correct per-event-kind repository AND applies the right pool-maintenance
// recipe: full upsert for swap/liquidity/genesis, last-seen touch otherwise,
// and the current-state projection only for swap/liquidity. All repositories
// (and the pool/projection repos behind `PoolMaintenance`) are mocked to
// append to an ordered call log — no database is involved.

#[cfg(test)]
#[path = "event_persistor_tests.rs"]
mod tests;
