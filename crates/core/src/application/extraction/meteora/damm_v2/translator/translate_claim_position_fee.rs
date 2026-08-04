//! Translation of `cp-amm::EvtClaimPositionFee` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtClaimPositionFee;
use crate::domain::{EventPosition, MeteoraDammV2ClaimPositionFeeEvent};

/// Translate an [`EvtClaimPositionFee`] into a [`MeteoraDammV2ClaimPositionFeeEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_position_fee(
    wire: &EvtClaimPositionFee,
    event_position: EventPosition,
) -> MeteoraDammV2ClaimPositionFeeEvent {
    MeteoraDammV2ClaimPositionFeeEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        position: wire.position,
        owner: wire.owner,
        fee_a_claimed: wire.fee_a_claimed,
        fee_b_claimed: wire.fee_b_claimed,
    }
}
