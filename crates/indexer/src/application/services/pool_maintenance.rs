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

    /// Upsert the pool with full information (mints known). Used by
    /// Swap and Liquidity events of any protocol.
    pub(crate) async fn upsert_pool_full(
        &self,
        protocol: Protocol,
        pool_address: solana_pubkey::Pubkey,
        token_a_mint: solana_pubkey::Pubkey,
        token_b_mint: solana_pubkey::Pubkey,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now();
        let pool = Pool {
            pool_address,
            protocol,
            token_a_mint,
            token_b_mint,
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
