//! Cross-protocol pool registry and `pool_current_state` projection, shared by every
//! per-protocol sub-persistor.

use std::sync::Arc;
use std::time::Instant;
use tracing::warn;
use yog_core::domain::{
    EventPosition, MeteoraDammV2LiquidityEvent, MeteoraDammV2SwapEvent, Pool,
    PoolCurrentStateRepository, PoolCurrentStateUpsert, PoolRepository, Protocol,
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

    /// Record a pool seen in the stream, with `None` mints: yog-context resolves them from the
    /// on-chain account.
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

    /// Flag a pool whose account-derived properties an event just invalidated, so yog-context
    /// re-reads its account; the indexer never writes a pool property itself.
    /// ⚠️ Returns its error: a lost flag leaves a property stale until the next event.
    pub(crate) async fn mark_needs_refresh(
        &self,
        protocol: Protocol,
        pool_address: &solana_pubkey::Pubkey,
    ) -> anyhow::Result<()> {
        let start = Instant::now();
        self.pool_repo.mark_needs_refresh(pool_address).await?;
        EventPersistorMetrics::record_persist_duration(
            &protocol,
            "pool_mark_needs_refresh",
            start.elapsed().as_secs_f64(),
        );
        Ok(())
    }

    /// Refresh `last_seen_at` for a pool; a no-op for an unknown one.
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

    /// Project a persisted DAMM v2 swap into `pool_current_state`; a failure is logged, never
    /// aborts the caller.
    pub(crate) async fn update_pool_current_state_from_swap(
        &self,
        protocol: Protocol,
        event: &MeteoraDammV2SwapEvent,
    ) {
        let upsert = PoolCurrentStateUpsert::from_swap(
            event.pool_address,
            protocol,
            EventPosition {
                signature: event.signature,
                timestamp: event.timestamp,
                slot: event.slot,
                transaction_index: event.transaction_index,
                event_index: event.event_index,
            },
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
            EventPosition {
                signature: event.signature,
                timestamp: event.timestamp,
                slot: event.slot,
                transaction_index: event.transaction_index,
                event_index: event.event_index,
            },
            event.liquidity_event_kind,
            event.reserve_a_after,
            event.reserve_b_after,
        );
        self.apply_pool_current_state_upsert(protocol, &upsert)
            .await;
    }

    /// Shared call site for the projection upsert. Records timing, classifies
    /// the outcome (`applied` vs `rejected`) as a metric label, and counts the
    /// case the ordering key cannot rank.
    async fn apply_pool_current_state_upsert(
        &self,
        protocol: Protocol,
        upsert: &PoolCurrentStateUpsert,
    ) {
        let start = Instant::now();
        match self.pool_current_state_repo.upsert(upsert).await {
            Ok(outcome) => {
                let label = if outcome.applied {
                    "pool_current_state_applied"
                } else {
                    "pool_current_state_rejected"
                };
                EventPersistorMetrics::record_persist_duration(
                    &protocol,
                    label,
                    start.elapsed().as_secs_f64(),
                );

                if outcome.same_slot_ambiguity {
                    // Counted on both outcomes: an ambiguity that wrongly accepts costs as much
                    // as one that wrongly rejects.
                    EventPersistorMetrics::record_pool_current_state_same_slot(&protocol);
                }
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
