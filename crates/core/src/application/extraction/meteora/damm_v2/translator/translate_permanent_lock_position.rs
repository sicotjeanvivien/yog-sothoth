//! Translation of `cp-amm::EvtPermanentLockPosition` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtPermanentLockPosition;
use crate::domain::{EventPosition, MeteoraDammV2PermanentLockPositionEvent};

/// Translate an [`EvtPermanentLockPosition`] into a
/// [`MeteoraDammV2PermanentLockPositionEvent`]. Infallible.
pub(super) fn translate_permanent_lock_position(
    wire: &EvtPermanentLockPosition,
    event_position: EventPosition,
) -> MeteoraDammV2PermanentLockPositionEvent {
    MeteoraDammV2PermanentLockPositionEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        position: wire.position,
        lock_liquidity_amount: wire.lock_liquidity_amount,
        total_permanent_locked_liquidity: wire.total_permanent_locked_liquidity,
    }
}
