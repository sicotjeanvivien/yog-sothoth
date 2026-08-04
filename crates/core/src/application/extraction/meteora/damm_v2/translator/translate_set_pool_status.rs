//! Translation of `cp-amm::EvtSetPoolStatus` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtSetPoolStatus;
use crate::domain::{EventPosition, MeteoraDammV2SetPoolStatusEvent};

/// Translate an [`EvtSetPoolStatus`] into a [`MeteoraDammV2SetPoolStatusEvent`].
/// Infallible.
pub(super) fn translate_set_pool_status(
    wire: &EvtSetPoolStatus,
    event_position: EventPosition,
) -> MeteoraDammV2SetPoolStatusEvent {
    MeteoraDammV2SetPoolStatusEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        status: wire.status,
    }
}
