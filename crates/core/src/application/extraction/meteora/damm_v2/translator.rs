//! Translate DAMM v2 wire events into protocol-agnostic domain events.
//!
//! Wire events ([`super::events::DammV2WireEvent`]) are byte-perfect mirrors
//! of cp-amm's on-chain Anchor events. Domain events
//! ([`crate::domain::DomainEvent`]) are protocol-agnostic representations
//! consumed by the indexer service.
//!
//! Token mints are NOT derived here: they are a property of the pool,
//! resolved authoritatively from the cp-amm Pool account by yog-context. Swap
//! and liquidity events therefore carry no mints — they reference the pool.

use crate::domain::EventPosition;
use crate::error::TranslationError;

use super::events::DammV2WireEvent;

mod translate_claim_position_fee;
mod translate_claim_protocol_fee;
mod translate_claim_reward;
mod translate_close_position;
mod translate_create_position;
mod translate_fund_reward;
mod translate_initialize_pool;
mod translate_initialize_reward;
mod translate_liquidity;
mod translate_lock_position;
mod translate_permanent_lock_position;
mod translate_set_pool_status;
mod translate_split_position;
mod translate_swap;
mod translate_update_pool_fees;
mod translate_update_reward_duration;
mod translate_update_reward_funder;
mod translate_withdraw_dead_liquidity_reward;
mod translate_withdraw_ineligible_reward;

use translate_claim_position_fee::translate_claim_position_fee;
use translate_claim_protocol_fee::translate_claim_protocol_fee;
use translate_claim_reward::translate_claim_reward;
use translate_close_position::translate_close_position;
use translate_create_position::translate_create_position;
use translate_fund_reward::translate_fund_reward;
use translate_initialize_pool::translate_initialize_pool;
use translate_initialize_reward::translate_initialize_reward;
use translate_liquidity::translate_liquidity;
use translate_lock_position::translate_lock_position;
use translate_permanent_lock_position::translate_permanent_lock_position;
use translate_set_pool_status::translate_set_pool_status;
use translate_split_position::translate_split_position;
use translate_swap::translate_swap;
// Swap-specific helper, re-exported only so `translator_tests.rs` — which reaches
// it through `use super::*` — keeps compiling now that it lives with translate_swap.
#[cfg(test)]
use translate_swap::compute_fee_token_is_a;
use translate_update_pool_fees::translate_update_pool_fees;
use translate_update_reward_duration::translate_update_reward_duration;
use translate_update_reward_funder::translate_update_reward_funder;
use translate_withdraw_dead_liquidity_reward::translate_withdraw_dead_liquidity_reward;
use translate_withdraw_ineligible_reward::translate_withdraw_ineligible_reward;

// ---------------------------------------------------------------------------
// High-level dispatch
// ---------------------------------------------------------------------------

/// Translate a single wire event into a domain event.
pub(super) fn translate_wire_event(
    wire: &DammV2WireEvent,
    event_position: EventPosition,
) -> Result<crate::domain::DomainEvent, TranslationError> {
    use crate::domain::DomainEvent;
    use crate::domain::MeteoraDammV2Event;

    let damm_v2_event = match wire {
        DammV2WireEvent::Swap2(e) => MeteoraDammV2Event::Swap(translate_swap(e, event_position)?),
        DammV2WireEvent::LiquidityChange(e) => {
            MeteoraDammV2Event::Liquidity(translate_liquidity(e, event_position)?)
        }
        DammV2WireEvent::ClaimPositionFee(e) => {
            MeteoraDammV2Event::ClaimPositionFee(translate_claim_position_fee(e, event_position))
        }
        DammV2WireEvent::ClaimReward(e) => {
            MeteoraDammV2Event::ClaimReward(translate_claim_reward(e, event_position))
        }
        DammV2WireEvent::ClaimProtocolFee(e) => {
            MeteoraDammV2Event::ClaimProtocolFee(translate_claim_protocol_fee(e, event_position))
        }
        DammV2WireEvent::InitializeReward(e) => {
            MeteoraDammV2Event::InitializeReward(translate_initialize_reward(e, event_position))
        }
        DammV2WireEvent::FundReward(e) => {
            MeteoraDammV2Event::FundReward(translate_fund_reward(e, event_position))
        }
        DammV2WireEvent::WithdrawIneligibleReward(e) => {
            MeteoraDammV2Event::WithdrawIneligibleReward(translate_withdraw_ineligible_reward(
                e,
                event_position,
            ))
        }
        DammV2WireEvent::UpdateRewardDuration(e) => MeteoraDammV2Event::UpdateRewardDuration(
            translate_update_reward_duration(e, event_position),
        ),
        DammV2WireEvent::UpdateRewardFunder(e) => MeteoraDammV2Event::UpdateRewardFunder(
            translate_update_reward_funder(e, event_position),
        ),
        DammV2WireEvent::WithdrawDeadLiquidityReward(e) => {
            MeteoraDammV2Event::WithdrawDeadLiquidityReward(
                translate_withdraw_dead_liquidity_reward(e, event_position),
            )
        }
        DammV2WireEvent::SplitPosition3(e) => {
            MeteoraDammV2Event::SplitPosition(translate_split_position(e, event_position))
        }
        DammV2WireEvent::CreatePosition(e) => {
            MeteoraDammV2Event::CreatePosition(translate_create_position(e, event_position))
        }
        DammV2WireEvent::ClosePosition(e) => {
            MeteoraDammV2Event::ClosePosition(translate_close_position(e, event_position))
        }
        DammV2WireEvent::LockPosition(e) => {
            MeteoraDammV2Event::LockPosition(translate_lock_position(e, event_position))
        }
        DammV2WireEvent::PermanentLockPosition(e) => MeteoraDammV2Event::PermanentLockPosition(
            translate_permanent_lock_position(e, event_position),
        ),
        DammV2WireEvent::InitializePool(e) => {
            MeteoraDammV2Event::InitializePool(translate_initialize_pool(e, event_position))
        }
        DammV2WireEvent::SetPoolStatus(e) => {
            MeteoraDammV2Event::SetPoolStatus(translate_set_pool_status(e, event_position))
        }
        DammV2WireEvent::UpdatePoolFees(e) => {
            MeteoraDammV2Event::UpdatePoolFees(translate_update_pool_fees(e, event_position))
        }
    };

    Ok(DomainEvent::MeteoraDammV2(damm_v2_event))
}

// ---------------------------------------------------------------------------
// Translation unit tests
// ---------------------------------------------------------------------------
//
// Field-mapping guards for the ring-2 lifecycle events that have no on-chain
// fixture yet (close / lock / permanent-lock / set-pool-status). They build a
// wire event with a distinct sentinel per field and assert each lands in the
// right domain field — catching swaps/typos in the translator. They do NOT
// validate the borsh layout or the discriminator against the real program;
// that still needs a fixture.

#[cfg(test)]
#[path = "translator_tests.rs"]
mod tests;
