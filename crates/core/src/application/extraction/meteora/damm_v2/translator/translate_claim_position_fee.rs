//! Translation of `cp-amm::EvtClaimPositionFee` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtClaimPositionFee;
use crate::domain::MeteoraDammV2ClaimPositionFeeEvent;

/// Translate an [`EvtClaimPositionFee`] into a [`MeteoraDammV2ClaimPositionFeeEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_position_fee(
    wire: &EvtClaimPositionFee,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClaimPositionFeeEvent {
    MeteoraDammV2ClaimPositionFeeEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        owner: wire.owner,
        fee_a_claimed: wire.fee_a_claimed,
        fee_b_claimed: wire.fee_b_claimed,
    }
}
