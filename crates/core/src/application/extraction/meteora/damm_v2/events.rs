//! Borsh mirrors of the Anchor events of Meteora DAMM v2 (`cp-amm`,
//! `programs/cp-amm/src/event.rs`); a program upgrade changes them in lockstep.
//!
//! ⚠️ `EvtSwap2` is not emitted by `emit_cpi!`: swap runs on pinocchio, and
//! `p_emit_cpi` rebuilds the same bytes by hand. A grep for `emit_cpi!` misses it.

use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::application::extraction::DISCRIMINATOR_LEN;

mod evt_claim_position_fee;
mod evt_claim_protocol_fee;
mod evt_claim_reward;
mod evt_close_position;
mod evt_create_position;
mod evt_fund_reward;
mod evt_initialize_pool;
mod evt_initialize_reward;
mod evt_liquidity_change;
mod evt_lock_position;
mod evt_permanent_lock_position;
mod evt_set_pool_status;
mod evt_split_position3;
mod evt_swap2;
mod evt_update_pool_fees;
mod evt_update_reward_duration;
mod evt_update_reward_funder;
mod evt_withdraw_dead_liquidity_reward;
mod evt_withdraw_ineligible_reward;

pub use evt_claim_position_fee::{EvtClaimPositionFee, discriminator_claim_position_fee};
pub use evt_claim_protocol_fee::{EvtClaimProtocolFee, discriminator_claim_protocol_fee};
pub use evt_claim_reward::{EvtClaimReward, discriminator_claim_reward};
pub use evt_close_position::{EvtClosePosition, discriminator_close_position};
pub use evt_create_position::{EvtCreatePosition, discriminator_create_position};
pub use evt_fund_reward::{EvtFundReward, discriminator_fund_reward};
pub use evt_initialize_pool::{
    BaseFeeParameters, DynamicFeeParameters, EvtInitializePool, PoolFeeParameters,
    discriminator_initialize_pool,
};
pub use evt_initialize_reward::{EvtInitializeReward, discriminator_initialize_reward};
pub use evt_liquidity_change::{EvtLiquidityChange, discriminator_liquidity_change};
pub use evt_lock_position::{EvtLockPosition, discriminator_lock_position};
pub use evt_permanent_lock_position::{
    EvtPermanentLockPosition, discriminator_permanent_lock_position,
};
pub use evt_set_pool_status::{EvtSetPoolStatus, discriminator_set_pool_status};
pub use evt_split_position3::{
    EvtSplitPosition3, SplitAmountInfo2, SplitPositionInfo2, SplitPositionParameters3,
    discriminator_split_position2, discriminator_split_position3,
};
pub use evt_swap2::{EvtSwap2, SwapParameters2, SwapResult2, discriminator_swap2};
pub use evt_update_pool_fees::{EvtUpdatePoolFees, discriminator_update_pool_fees};
pub use evt_update_reward_duration::{
    EvtUpdateRewardDuration, discriminator_update_reward_duration,
};
pub use evt_update_reward_funder::{EvtUpdateRewardFunder, discriminator_update_reward_funder};
pub use evt_withdraw_dead_liquidity_reward::{
    EvtWithdrawDeadLiquidityReward, discriminator_withdraw_dead_liquidity_reward,
};
pub use evt_withdraw_ineligible_reward::{
    EvtWithdrawIneligibleReward, discriminator_withdraw_ineligible_reward,
};

// ---------------------------------------------------------------------------
// Discriminator helper
// ---------------------------------------------------------------------------

/// Anchor event discriminator: `sha256("event:<name>")[..8]`.
fn compute_discriminator(name: &str) -> [u8; DISCRIMINATOR_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(format!("event:{name}").as_bytes());
    let full = hasher.finalize();
    let mut out = [0u8; DISCRIMINATOR_LEN];
    out.copy_from_slice(&full[..DISCRIMINATOR_LEN]);
    out
}

// ---------------------------------------------------------------------------
// Wire event sum type
// ---------------------------------------------------------------------------

/// One decoded DAMM v2 Anchor event; the variants are the whole set the extractor
/// dispatches on, anything else is an unknown discriminator.
#[derive(Debug, Clone)]
pub enum DammV2WireEvent {
    Swap2(EvtSwap2),
    LiquidityChange(EvtLiquidityChange),
    ClaimPositionFee(EvtClaimPositionFee),
    ClaimReward(EvtClaimReward),
    ClaimProtocolFee(EvtClaimProtocolFee),
    InitializeReward(EvtInitializeReward),
    FundReward(EvtFundReward),
    WithdrawIneligibleReward(EvtWithdrawIneligibleReward),
    UpdateRewardDuration(EvtUpdateRewardDuration),
    UpdateRewardFunder(EvtUpdateRewardFunder),
    WithdrawDeadLiquidityReward(EvtWithdrawDeadLiquidityReward),
    /// Boxed: its payload dwarfs every other variant.
    SplitPosition3(Box<EvtSplitPosition3>),
    CreatePosition(EvtCreatePosition),
    ClosePosition(EvtClosePosition),
    LockPosition(EvtLockPosition),
    PermanentLockPosition(EvtPermanentLockPosition),
    /// Boxed: the genesis payload dwarfs every other variant.
    InitializePool(Box<EvtInitializePool>),
    SetPoolStatus(EvtSetPoolStatus),
    UpdatePoolFees(EvtUpdatePoolFees),
}

impl DammV2WireEvent {
    /// Pool the event refers to.
    pub fn pool(&self) -> Pubkey {
        match self {
            Self::Swap2(e) => e.pool,
            Self::LiquidityChange(e) => e.pool,
            Self::ClaimPositionFee(e) => e.pool,
            Self::ClaimReward(e) => e.pool,
            Self::ClaimProtocolFee(e) => e.pool,
            Self::InitializeReward(e) => e.pool,
            Self::FundReward(e) => e.pool,
            Self::WithdrawIneligibleReward(e) => e.pool,
            Self::UpdateRewardDuration(e) => e.pool,
            Self::UpdateRewardFunder(e) => e.pool,
            Self::WithdrawDeadLiquidityReward(e) => e.pool,
            Self::SplitPosition3(e) => e.pool,
            Self::CreatePosition(e) => e.pool,
            Self::ClosePosition(e) => e.pool,
            Self::LockPosition(e) => e.pool,
            Self::PermanentLockPosition(e) => e.pool,
            Self::InitializePool(e) => e.pool,
            Self::SetPoolStatus(e) => e.pool,
            Self::UpdatePoolFees(e) => e.pool,
        }
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
