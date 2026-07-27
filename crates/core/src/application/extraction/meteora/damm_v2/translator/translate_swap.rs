//! Translation of `cp-amm::EvtSwap2` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtSwap2;
use crate::domain::{MeteoraDammV2SwapEvent, TradeDirection};
use crate::error::TranslationError;

/// Translate an [`EvtSwap2`] into a [`MeteoraDammV2SwapEvent`].
///
/// Returns `Err` only if `trade_direction` is invalid (out of range).
pub(super) fn translate_swap(
    wire: &EvtSwap2,
    signature: Signature,
    timestamp: DateTime<Utc>,
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

    // EvtSwap2 reports input/output amounts in
    // `included_transfer_fee_amount_in` / `included_transfer_fee_amount_out`.
    // Map them onto (amount_a, amount_b) according to trade direction:
    //   AtoB → input is on a, output is on b
    //   BtoA → input is on b, output is on a
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
        signature,
        timestamp,

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

/// Determine whether the fee was charged on token A (`true`) or token B
/// (`false`), based on the on-chain `collect_fee_mode` and the swap's
/// `trade_direction`.
///
/// Mirrors `cp-amm::FeeMode::get_fee_mode` — see source comments in
/// `cp-amm/programs/cp-amm/src/state/fee.rs`. Updated alongside cp-amm
/// upgrades.
pub(super) fn compute_fee_token_is_a(
    collect_fee_mode: u8,
    trade_direction: TradeDirection,
) -> Result<bool, u8> {
    // CollectFeeMode mapping (mirrors cp-amm enum):
    //   0 = BothToken    — fee on the OUT token
    //   1 = OnlyB        — fee always on token B
    //   2 = Compounding  — fee always on token B
    let fee_token_is_a = match (collect_fee_mode, trade_direction) {
        (0, TradeDirection::AtoB) => false, // out is B, fee on B
        (0, TradeDirection::BtoA) => true,  // out is A, fee on A
        (1, _) => false,                    // OnlyB → always B
        (2, _) => false,                    // Compounding → always B
        (other, _) => return Err(other),
    };
    Ok(fee_token_is_a)
}
