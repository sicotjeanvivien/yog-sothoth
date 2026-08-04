//! Translation of `cp-amm::EvtUpdatePoolFees` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtUpdatePoolFees;
use crate::domain::{EventPosition, MeteoraDammV2UpdatePoolFeesEvent};

/// Translate an [`EvtUpdatePoolFees`] into a [`MeteoraDammV2UpdatePoolFeesEvent`].
/// Infallible — the fee params are carried through as the raw, undecoded blob.
pub(super) fn translate_update_pool_fees(
    wire: &EvtUpdatePoolFees,
    event_position: EventPosition,
) -> MeteoraDammV2UpdatePoolFeesEvent {
    MeteoraDammV2UpdatePoolFeesEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        operator: wire.operator,
        params_raw: wire.params_raw.clone(),
    }
}
