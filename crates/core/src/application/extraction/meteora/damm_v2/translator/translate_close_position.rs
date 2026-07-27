//! Translation of `cp-amm::EvtClosePosition` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtClosePosition;
use crate::domain::MeteoraDammV2ClosePositionEvent;

/// Translate an [`EvtClosePosition`] into a [`MeteoraDammV2ClosePositionEvent`].
///
/// Infallible — self-contained wire event, no transferChecked context needed.
pub(super) fn translate_close_position(
    wire: &EvtClosePosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClosePositionEvent {
    MeteoraDammV2ClosePositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        owner: wire.owner,
        position: wire.position,
        position_nft_mint: wire.position_nft_mint,
    }
}
