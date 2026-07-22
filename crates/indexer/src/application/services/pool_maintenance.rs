//! Cross-protocol pool registry and pool_current_state projection helper.
//!
//! Pool maintenance is intrinsically cross-protocol: every protocol
//! upserts pools into the same `pools` table and refreshes the same
//! `pool_current_state` projection. This struct centralises that logic
//! so each per-protocol sub-persistor depends on a single shared helper
//! instead of duplicating it.

use std::sync::Arc;
use std::time::Instant;
use tracing::warn;
use yog_core::domain::{
    MeteoraDammV2LiquidityEvent, MeteoraDammV2SwapEvent, Pool, PoolCurrentStateRepository,
    PoolCurrentStateUpsert, PoolRepository, Protocol,
};

use crate::application::services::EventPersistorMetrics;

pub(crate) struct PoolMaintenance {
    pool_repo: Arc<dyn PoolRepository>,
    pool_current_state_repo: Arc<dyn PoolCurrentStateRepository>,
}

impl PoolMaintenance {
    pub(crate) fn new(
        pool_repo: Arc<dyn PoolRepository>,
        pool_current_state_repo: Arc<dyn PoolCurrentStateRepository>,
    ) -> Self {
        Self {
            pool_repo,
            pool_current_state_repo,
        }
    }

    /// Record a pool seen in the stream. The mints are NOT known here —
    /// they're a pool property resolved later from the on-chain pool account
    /// by yog-context, so the row is created with `None` mints. Used by Swap
    /// and Liquidity events of any protocol.
    pub(crate) async fn discover_pool(
        &self,
        protocol: Protocol,
        pool_address: solana_pubkey::Pubkey,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now();
        let pool = Pool {
            pool_address,
            protocol,
            token_a_mint: None,
            token_b_mint: None,
            fee_bps: None,
            protocol_fee_percent: None,
            partner_fee_percent: None,
            referral_fee_percent: None,
            first_seen_at: now,
            last_seen_at: now,
        };
        let start = Instant::now();
        self.pool_repo.upsert(&pool).await?;
        EventPersistorMetrics::record_persist_duration(
            &protocol,
            "pool_upsert",
            start.elapsed().as_secs_f64(),
        );
        Ok(())
    }

    /// Record a pool's base trading fee (basis points), decoded from its
    /// genesis fee config. Best-effort: a failure here is logged but never
    /// aborts the caller — the pool simply keeps a NULL `fee_bps`.
    pub(crate) async fn set_fee_bps(
        &self,
        protocol: Protocol,
        pool_address: &solana_pubkey::Pubkey,
        fee_bps: rust_decimal::Decimal,
    ) {
        let start = Instant::now();
        match self.pool_repo.set_fee_bps(pool_address, fee_bps).await {
            Ok(()) => {
                EventPersistorMetrics::record_persist_duration(
                    &protocol,
                    "pool_set_fee_bps",
                    start.elapsed().as_secs_f64(),
                );
            }
            Err(err) => {
                warn!(
                    protocol = %protocol.as_str(),
                    error = %err,
                    "pool set_fee_bps failed"
                );
            }
        }
    }

    /// Record a pool's decoded fee *shape* (base-fee kind + dynamic-fee flag),
    /// decoded from its genesis fee config alongside the fee tier. Best-effort:
    /// a failure here is logged but never aborts the caller — the pool simply
    /// keeps NULL fee-shape columns.
    pub(crate) async fn set_fee_config(
        &self,
        protocol: Protocol,
        pool_address: &solana_pubkey::Pubkey,
        base_fee_kind: &str,
        has_dynamic_fee: bool,
    ) {
        let start = Instant::now();
        match self
            .pool_repo
            .set_fee_config(pool_address, base_fee_kind, has_dynamic_fee)
            .await
        {
            Ok(()) => {
                EventPersistorMetrics::record_persist_duration(
                    &protocol,
                    "pool_set_fee_config",
                    start.elapsed().as_secs_f64(),
                );
            }
            Err(err) => {
                warn!(
                    protocol = %protocol.as_str(),
                    error = %err,
                    "pool set_fee_config failed"
                );
            }
        }
    }

    /// Refresh `last_seen_at` for a pool. No-op if the pool is unknown
    /// (will be created when a Swap or Liquidity event arrives later).
    /// Used by ClaimPositionFee and ClaimReward events of any protocol.
    pub(crate) async fn touch_pool(
        &self,
        protocol: Protocol,
        pool_address: &solana_pubkey::Pubkey,
    ) {
        let start = Instant::now();
        match self.pool_repo.touch_last_seen(pool_address).await {
            Ok(()) => {
                EventPersistorMetrics::record_persist_duration(
                    &protocol,
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

    /// Project a freshly-persisted DAMM v2 swap event into
    /// `pool_current_state`. Best-effort: a failure here is logged but
    /// never aborts the caller.
    pub(crate) async fn update_pool_current_state_from_swap(
        &self,
        protocol: Protocol,
        event: &MeteoraDammV2SwapEvent,
    ) {
        let upsert = PoolCurrentStateUpsert::from_swap(
            event.pool_address,
            protocol,
            event.timestamp,
            event.signature,
            event.reserve_a_after,
            event.reserve_b_after,
            event.next_sqrt_price,
        );
        self.apply_pool_current_state_upsert(protocol, &upsert)
            .await;
    }

    /// Project a freshly-persisted DAMM v2 liquidity event into
    /// `pool_current_state`.
    pub(crate) async fn update_pool_current_state_from_liquidity(
        &self,
        protocol: Protocol,
        event: &MeteoraDammV2LiquidityEvent,
    ) {
        let upsert = PoolCurrentStateUpsert::from_liquidity(
            event.pool_address,
            protocol,
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
    /// classifies the outcome (`applied` vs `stale`) as a metric label
    /// so stale-write rates can be observed in Prometheus.
    async fn apply_pool_current_state_upsert(
        &self,
        protocol: Protocol,
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
                    &protocol,
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
