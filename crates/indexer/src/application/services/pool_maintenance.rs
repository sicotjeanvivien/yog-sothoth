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

    /// Flag a pool whose account-derived properties an event just invalidated,
    /// so yog-context re-reads its on-chain account.
    ///
    /// **The indexer's only way to affect a pool property.** It does not decode
    /// the new value and store it: those columns have a single writer, and an
    /// update event carries a *delta* — variable-offset borsh tags, `Option`s
    /// encoding three states — where the account carries resolved state at fixed
    /// offsets.
    ///
    /// Unlike the writes it replaces this returns its error rather than
    /// swallowing it: a lost flag is a property that stays stale until the next
    /// event happens to touch the pool, so the caller decides how loudly to say
    /// so.
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

    // NOTE: this crate no longer writes any pool *property*.
    //
    // `set_fee_bps` lived here and wrote the base fee decoded from a genesis or
    // fee-update event; `set_fee_config` lived here before migration 036 moved
    // it to the DAMM v2 sub-persistor. Both are gone: `pools` and the satellites
    // have a single writer, yog-context, and this crate flags a pool for refresh
    // instead. What remains here is identity and observation — `discover_pool`
    // and `touch_pool` — which are facts about *seeing* a pool, not about the
    // state of its on-chain account.

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
                // `rejected`, not `stale`: the old label asserted a cause —
                // healthy concurrency — for what was mostly the guard's own
                // second-granularity. It states the fact and stops there.
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
                    // Counted on BOTH paths — see the outcome's doc-comment.
                    // This is the measurement that will say whether the
                    // residual case is as rare as it was estimated to be; if
                    // it is not, `getBlock` comes back on the table.
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
