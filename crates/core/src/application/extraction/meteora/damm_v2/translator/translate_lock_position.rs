//! Translation of `cp-amm::EvtLockPosition` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtLockPosition;
use crate::domain::{EventPosition, MeteoraDammV2LockPositionEvent};

/// Translate an [`EvtLockPosition`] into a [`MeteoraDammV2LockPositionEvent`].
///
/// Infallible — every field maps directly, no enum or context to resolve.
pub(super) fn translate_lock_position(
    wire: &EvtLockPosition,
    event_position: EventPosition,
) -> MeteoraDammV2LockPositionEvent {
    MeteoraDammV2LockPositionEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        position: wire.position,
        owner: wire.owner,
        vesting: wire.vesting,
        cliff_point: wire.cliff_point,
        period_frequency: wire.period_frequency,
        cliff_unlock_liquidity: wire.cliff_unlock_liquidity,
        liquidity_per_period: wire.liquidity_per_period,
        number_of_period: wire.number_of_period,
    }
}
