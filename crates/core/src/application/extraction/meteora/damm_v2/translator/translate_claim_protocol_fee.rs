//! Translation of `cp-amm::EvtClaimProtocolFee` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtClaimProtocolFee;
use crate::domain::MeteoraDammV2ClaimProtocolFeeEvent;

/// Translate an [`EvtClaimProtocolFee`] into a [`MeteoraDammV2ClaimProtocolFeeEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_protocol_fee(
    wire: &EvtClaimProtocolFee,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClaimProtocolFeeEvent {
    MeteoraDammV2ClaimProtocolFeeEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        token_a_amount: wire.token_a_amount,
        token_b_amount: wire.token_b_amount,
    }
}
