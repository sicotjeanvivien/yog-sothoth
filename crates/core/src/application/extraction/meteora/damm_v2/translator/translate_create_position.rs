//! Translation of `cp-amm::EvtCreatePosition` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtCreatePosition;
use crate::domain::MeteoraDammV2CreatePositionEvent;

/// Translate an [`EvtCreatePosition`] into a [`MeteoraDammV2CreatePositionEvent`].
///
/// This translation is infallible — the wire event is self-contained
/// (pool, owner, position, position NFT mint), so no transferChecked
/// context is required.
pub(super) fn translate_create_position(
    wire: &EvtCreatePosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2CreatePositionEvent {
    MeteoraDammV2CreatePositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        owner: wire.owner,
        position: wire.position,
        position_nft_mint: wire.position_nft_mint,
    }
}
