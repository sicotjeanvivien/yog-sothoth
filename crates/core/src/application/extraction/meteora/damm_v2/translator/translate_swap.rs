//! Translation of `cp-amm::EvtSwap2` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtSwap2;
use crate::domain::{EventPosition, MeteoraDammV2SwapEvent, TradeDirection};
use crate::error::TranslationError;

/// Translate an [`EvtSwap2`] into a [`MeteoraDammV2SwapEvent`].
///
/// Returns `Err` only if `trade_direction` is invalid (out of range).
pub(super) fn translate_swap(
    wire: &EvtSwap2,
    event_position: EventPosition,
) -> Result<MeteoraDammV2SwapEvent, TranslationError> {
    let trade_direction = TradeDirection::from_u8(wire.trade_direction).map_err(|raw| {
        TranslationError::InvalidEnum {
            field: "trade_direction",
            value: raw,
        }
    })?;

    let fee_token_is_a =
        compute_fee_token_is_a(wire.collect_fee_mode, trade_direction).map_err(|raw| {
            TranslationError::InvalidEnum {
                field: "collect_fee_mode",
                value: raw,
            }
        })?;

    // Input and output onto (amount_a, amount_b): AtoB → (in, out), BtoA → (out, in).
    let (amount_a, amount_b) = match trade_direction {
        TradeDirection::AtoB => (
            wire.included_transfer_fee_amount_in,
            wire.included_transfer_fee_amount_out,
        ),
        TradeDirection::BtoA => (
            wire.included_transfer_fee_amount_out,
            wire.included_transfer_fee_amount_in,
        ),
    };
    Ok(MeteoraDammV2SwapEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,

        trade_direction,
        amount_a,
        amount_b,

        reserve_a_after: wire.reserve_a_amount,
        reserve_b_after: wire.reserve_b_amount,
        next_sqrt_price: wire.swap_result.next_sqrt_price,

        claiming_fee: wire.swap_result.claiming_fee,
        protocol_fee: wire.swap_result.protocol_fee,
        compounding_fee: wire.swap_result.compounding_fee,
        referral_fee: wire.swap_result.referral_fee,
        fee_token_is_a,
    })
}

/// Whether the fee was charged on token A (`true`) or B, as cp-amm's
/// `FeeMode::get_fee_mode` (`state/fee.rs`) decides it.
pub(super) fn compute_fee_token_is_a(
    collect_fee_mode: u8,
    trade_direction: TradeDirection,
) -> Result<bool, u8> {
    // 0 BothToken: fee on the output token; 1 OnlyB and 2 Compounding: always on B.
    let fee_token_is_a = match (collect_fee_mode, trade_direction) {
        (0, TradeDirection::AtoB) => false, // out is B, fee on B
        (0, TradeDirection::BtoA) => true,  // out is A, fee on A
        (1, _) => false,                    // OnlyB → always B
        (2, _) => false,                    // Compounding → always B
        (other, _) => return Err(other),
    };
    Ok(fee_token_is_a)
}
