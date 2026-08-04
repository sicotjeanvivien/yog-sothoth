//! Translation of `cp-amm::EvtCreatePosition` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtCreatePosition;
use crate::domain::{EventPosition, MeteoraDammV2CreatePositionEvent};

/// Translate an [`EvtCreatePosition`] into a [`MeteoraDammV2CreatePositionEvent`].
///
/// This translation is infallible — the wire event is self-contained
/// (pool, owner, position, position NFT mint), so no transferChecked
/// context is required.
pub(super) fn translate_create_position(
    wire: &EvtCreatePosition,
    event_position: EventPosition,
) -> MeteoraDammV2CreatePositionEvent {
    MeteoraDammV2CreatePositionEvent {
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
