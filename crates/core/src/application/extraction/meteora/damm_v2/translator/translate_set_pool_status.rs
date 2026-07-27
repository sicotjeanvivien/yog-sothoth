//! Translation of `cp-amm::EvtSetPoolStatus` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtSetPoolStatus;
use crate::domain::MeteoraDammV2SetPoolStatusEvent;

/// Translate an [`EvtSetPoolStatus`] into a [`MeteoraDammV2SetPoolStatusEvent`].
/// Infallible.
pub(super) fn translate_set_pool_status(
    wire: &EvtSetPoolStatus,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2SetPoolStatusEvent {
    MeteoraDammV2SetPoolStatusEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        status: wire.status,
    }
}
