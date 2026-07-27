//! Translation of `cp-amm::EvtSplitPosition3` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::{
    EvtSplitPosition3, SplitPositionInfo2,
};
use crate::domain::{
    MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators, MeteoraDammV2SplitPositionEvent,
    MeteoraDammV2SplitPositionState,
};

/// Translate an [`EvtSplitPosition3`] into a [`MeteoraDammV2SplitPositionEvent`].
///
/// Infallible. The three nested wire sub-structs map one-to-one onto their
/// domain counterparts; note that `amount_splits` and the two `*_position_info`
/// carry their leading `u128`s in a *different order* on the wire, which is why
/// they stay separate types on both sides.
pub(super) fn translate_split_position(
    wire: &EvtSplitPosition3,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2SplitPositionEvent {
    let state = |i: &SplitPositionInfo2| MeteoraDammV2SplitPositionState {
        unlocked_liquidity: i.unlocked_liquidity,
        permanent_locked_liquidity: i.permanent_locked_liquidity,
        vested_liquidity: i.vested_liquidity,
        fee_a: i.fee_a,
        fee_b: i.fee_b,
        reward_0: i.reward_0,
        reward_1: i.reward_1,
    };
    let a = &wire.amount_splits;
    let p = &wire.split_position_parameters;
    MeteoraDammV2SplitPositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        first_owner: wire.first_owner,
        second_owner: wire.second_owner,
        first_position: wire.first_position,
        second_position: wire.second_position,
        current_sqrt_price: wire.current_sqrt_price,
        amounts: MeteoraDammV2SplitAmounts {
            permanent_locked_liquidity: a.permanent_locked_liquidity,
            unlocked_liquidity: a.unlocked_liquidity,
            vested_liquidity: a.vested_liquidity,
            fee_a: a.fee_a,
            fee_b: a.fee_b,
            reward_0: a.reward_0,
            reward_1: a.reward_1,
        },
        first_position_after: state(&wire.first_position_info),
        second_position_after: state(&wire.second_position_info),
        numerators: MeteoraDammV2SplitNumerators {
            unlocked_liquidity: p.unlocked_liquidity_numerator,
            permanent_locked_liquidity: p.permanent_locked_liquidity_numerator,
            fee_a: p.fee_a_numerator,
            fee_b: p.fee_b_numerator,
            reward_0: p.reward_0_numerator,
            reward_1: p.reward_1_numerator,
            inner_vesting_liquidity: p.inner_vesting_liquidity_numerator,
        },
    }
}
