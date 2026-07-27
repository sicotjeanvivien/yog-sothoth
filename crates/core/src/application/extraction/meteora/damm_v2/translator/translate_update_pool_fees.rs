//! Translation of `cp-amm::EvtUpdatePoolFees` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtUpdatePoolFees;
use crate::domain::MeteoraDammV2UpdatePoolFeesEvent;

/// Translate an [`EvtUpdatePoolFees`] into a [`MeteoraDammV2UpdatePoolFeesEvent`].
/// Infallible — the fee params are carried through as the raw, undecoded blob.
pub(super) fn translate_update_pool_fees(
    wire: &EvtUpdatePoolFees,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2UpdatePoolFeesEvent {
    MeteoraDammV2UpdatePoolFeesEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        operator: wire.operator,
        params_raw: wire.params_raw.clone(),
    }
}
