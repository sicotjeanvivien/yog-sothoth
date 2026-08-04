//! Translation of `cp-amm::EvtClaimProtocolFee` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtClaimProtocolFee;
use crate::domain::{EventPosition, MeteoraDammV2ClaimProtocolFeeEvent};

/// Translate an [`EvtClaimProtocolFee`] into a [`MeteoraDammV2ClaimProtocolFeeEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_protocol_fee(
    wire: &EvtClaimProtocolFee,
    event_position: EventPosition,
) -> MeteoraDammV2ClaimProtocolFeeEvent {
    MeteoraDammV2ClaimProtocolFeeEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        token_a_amount: wire.token_a_amount,
        token_b_amount: wire.token_b_amount,
    }
}
