//! Translation of `cp-amm::EvtPermanentLockPosition` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtPermanentLockPosition;
use crate::domain::MeteoraDammV2PermanentLockPositionEvent;

/// Translate an [`EvtPermanentLockPosition`] into a
/// [`MeteoraDammV2PermanentLockPositionEvent`]. Infallible.
pub(super) fn translate_permanent_lock_position(
    wire: &EvtPermanentLockPosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2PermanentLockPositionEvent {
    MeteoraDammV2PermanentLockPositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        lock_liquidity_amount: wire.lock_liquidity_amount,
        total_permanent_locked_liquidity: wire.total_permanent_locked_liquidity,
    }
}
