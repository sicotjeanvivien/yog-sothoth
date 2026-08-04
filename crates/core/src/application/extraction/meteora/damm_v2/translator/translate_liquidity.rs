//! Translation of `cp-amm::EvtLiquidityChange` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtLiquidityChange;
use crate::domain::{EventPosition, MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventKind};
use crate::error::TranslationError;

/// Translate an [`EvtLiquidityChange`] into a [`MeteoraDammV2LiquidityEvent`].
pub(super) fn translate_liquidity(
    wire: &EvtLiquidityChange,
    event_position: EventPosition,
) -> Result<MeteoraDammV2LiquidityEvent, TranslationError> {
    let liquidity_event_kind =
        MeteoraDammV2LiquidityEventKind::from_u8(wire.change_type).map_err(|raw| {
            TranslationError::InvalidEnum {
                field: "change_type",
                value: raw,
            }
        })?;

    Ok(MeteoraDammV2LiquidityEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,

        liquidity_event_kind,
        amount_a: wire.token_a_amount,
        amount_b: wire.token_b_amount,
        liquidity_delta: wire.liquidity_delta,

        reserve_a_after: wire.reserve_a_amount,
        reserve_b_after: wire.reserve_b_amount,

        position: wire.position,
        owner: wire.owner,
    })
}
