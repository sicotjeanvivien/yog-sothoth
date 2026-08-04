//! Translation of `cp-amm::EvtInitializePool` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtInitializePool;
use crate::domain::{EventPosition, MeteoraDammV2InitializePoolEvent};

/// Translate an [`EvtInitializePool`] into a [`MeteoraDammV2InitializePoolEvent`].
///
/// Self-contained — the wire event carries both mints, so no transferChecked
/// context is needed. The fee parameters are re-serialized to borsh and stored
/// raw (undecoded) under "voie C". `borsh::to_vec` into a `Vec` cannot fail in
/// practice (no I/O), so the `expect` is unreachable.
pub(super) fn translate_initialize_pool(
    wire: &EvtInitializePool,
    event_position: EventPosition,
) -> MeteoraDammV2InitializePoolEvent {
    let pool_fees_raw =
        borsh::to_vec(&wire.pool_fees).expect("borsh serialize to Vec is infallible");

    MeteoraDammV2InitializePoolEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        token_a_mint: wire.token_a_mint,
        token_b_mint: wire.token_b_mint,
        creator: wire.creator,
        payer: wire.payer,
        alpha_vault: wire.alpha_vault,
        sqrt_min_price: wire.sqrt_min_price,
        sqrt_max_price: wire.sqrt_max_price,
        sqrt_price: wire.sqrt_price,
        liquidity: wire.liquidity,
        activation_type: wire.activation_type,
        activation_point: wire.activation_point,
        collect_fee_mode: wire.collect_fee_mode,
        pool_type: wire.pool_type,
        token_a_flag: wire.token_a_flag,
        token_b_flag: wire.token_b_flag,
        token_a_amount: wire.token_a_amount,
        token_b_amount: wire.token_b_amount,
        total_amount_a: wire.total_amount_a,
        total_amount_b: wire.total_amount_b,
        pool_fees_raw,
    }
}
