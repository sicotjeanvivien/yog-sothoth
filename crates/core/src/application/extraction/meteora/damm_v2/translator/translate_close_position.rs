//! Translation of `cp-amm::EvtClosePosition` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtClosePosition;
use crate::domain::{EventPosition, MeteoraDammV2ClosePositionEvent};

/// Translate an [`EvtClosePosition`] into a [`MeteoraDammV2ClosePositionEvent`].
///
/// Infallible — self-contained wire event, no transferChecked context needed.
pub(super) fn translate_close_position(
    wire: &EvtClosePosition,
    event_position: EventPosition,
) -> MeteoraDammV2ClosePositionEvent {
    MeteoraDammV2ClosePositionEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        owner: wire.owner,
        position: wire.position,
        position_nft_mint: wire.position_nft_mint,
    }
}
