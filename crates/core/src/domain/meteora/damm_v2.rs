mod claim_position_fee_event;
mod claim_protocol_fee_event;
mod claim_reward_event;
mod close_position_event;
mod create_position_event;
mod fund_reward_event;
mod initialize_pool_event;
mod initialize_reward_event;
mod liquidity_event;
mod lock_position_event;
mod permanent_lock_position_event;
mod set_pool_status_event;
mod swap_event;
mod update_pool_fees_event;
mod update_reward_duration_event;
mod update_reward_funder_event;
mod withdraw_dead_liquidity_reward_event;
mod withdraw_ineligible_reward_event;

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

pub use claim_position_fee_event::{
    MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
};
pub use claim_protocol_fee_event::{
    MeteoraDammV2ClaimProtocolFeeEvent, MeteoraDammV2ClaimProtocolFeeEventRepository,
};
pub use claim_reward_event::{
    MeteoraDammV2ClaimRewardEvent, MeteoraDammV2ClaimRewardEventRepository,
};
pub use close_position_event::{
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2ClosePositionEventRepository,
};
pub use create_position_event::{
    MeteoraDammV2CreatePositionEvent, MeteoraDammV2CreatePositionEventRepository,
};
pub use fund_reward_event::{MeteoraDammV2FundRewardEvent, MeteoraDammV2FundRewardEventRepository};
pub use initialize_pool_event::{
    MeteoraDammV2InitializePoolEvent, MeteoraDammV2InitializePoolEventRepository,
};
pub use initialize_reward_event::{
    MeteoraDammV2InitializeRewardEvent, MeteoraDammV2InitializeRewardEventRepository,
};
pub use liquidity_event::{
    MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventCursor,
    MeteoraDammV2LiquidityEventFeed, MeteoraDammV2LiquidityEventKind,
    MeteoraDammV2LiquidityEventRepository, MeteoraDammV2LiquidityEventValued,
};
pub use lock_position_event::{
    MeteoraDammV2LockPositionEvent, MeteoraDammV2LockPositionEventRepository,
};
pub use permanent_lock_position_event::{
    MeteoraDammV2PermanentLockPositionEvent, MeteoraDammV2PermanentLockPositionEventRepository,
};
pub use set_pool_status_event::{
    MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2SetPoolStatusEventRepository,
};
pub use swap_event::{
    MeteoraDammV2SwapEvent, MeteoraDammV2SwapEventCursor, MeteoraDammV2SwapEventFeed,
    MeteoraDammV2SwapEventRepository,
};
pub use update_pool_fees_event::{
    MeteoraDammV2UpdatePoolFeesEvent, MeteoraDammV2UpdatePoolFeesEventRepository,
};
pub use update_reward_duration_event::{
    MeteoraDammV2UpdateRewardDurationEvent, MeteoraDammV2UpdateRewardDurationEventRepository,
};
pub use update_reward_funder_event::{
    MeteoraDammV2UpdateRewardFunderEvent, MeteoraDammV2UpdateRewardFunderEventRepository,
};
pub use withdraw_dead_liquidity_reward_event::{
    MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
};
pub use withdraw_ineligible_reward_event::{
    MeteoraDammV2WithdrawIneligibleRewardEvent,
    MeteoraDammV2WithdrawIneligibleRewardEventRepository,
};

/// Every kind of event the Meteora DAMM v2 extractor can produce, grouped
/// under a single sub-enum so [`crate::domain::DomainEvent`] can dispatch
/// at the protocol level first.
#[derive(Debug, Clone)]
pub enum MeteoraDammV2Event {
    Swap(MeteoraDammV2SwapEvent),
    Liquidity(MeteoraDammV2LiquidityEvent),
    ClaimPositionFee(MeteoraDammV2ClaimPositionFeeEvent),
    ClaimReward(MeteoraDammV2ClaimRewardEvent),
    ClaimProtocolFee(MeteoraDammV2ClaimProtocolFeeEvent),
    InitializeReward(MeteoraDammV2InitializeRewardEvent),
    FundReward(MeteoraDammV2FundRewardEvent),
    WithdrawIneligibleReward(MeteoraDammV2WithdrawIneligibleRewardEvent),
    UpdateRewardDuration(MeteoraDammV2UpdateRewardDurationEvent),
    UpdateRewardFunder(MeteoraDammV2UpdateRewardFunderEvent),
    WithdrawDeadLiquidityReward(MeteoraDammV2WithdrawDeadLiquidityRewardEvent),
    CreatePosition(MeteoraDammV2CreatePositionEvent),
    ClosePosition(MeteoraDammV2ClosePositionEvent),
    LockPosition(MeteoraDammV2LockPositionEvent),
    PermanentLockPosition(MeteoraDammV2PermanentLockPositionEvent),
    InitializePool(MeteoraDammV2InitializePoolEvent),
    SetPoolStatus(MeteoraDammV2SetPoolStatusEvent),
    UpdatePoolFees(MeteoraDammV2UpdatePoolFeesEvent),
}

impl MeteoraDammV2Event {
    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::Swap(e) => e.pool_address,
            Self::Liquidity(e) => e.pool_address,
            Self::ClaimPositionFee(e) => e.pool_address,
            Self::ClaimReward(e) => e.pool_address,
            Self::ClaimProtocolFee(e) => e.pool_address,
            Self::InitializeReward(e) => e.pool_address,
            Self::FundReward(e) => e.pool_address,
            Self::WithdrawIneligibleReward(e) => e.pool_address,
            Self::UpdateRewardDuration(e) => e.pool_address,
            Self::UpdateRewardFunder(e) => e.pool_address,
            Self::WithdrawDeadLiquidityReward(e) => e.pool_address,
            Self::CreatePosition(e) => e.pool_address,
            Self::ClosePosition(e) => e.pool_address,
            Self::LockPosition(e) => e.pool_address,
            Self::PermanentLockPosition(e) => e.pool_address,
            Self::InitializePool(e) => e.pool_address,
            Self::SetPoolStatus(e) => e.pool_address,
            Self::UpdatePoolFees(e) => e.pool_address,
        }
    }

    pub fn signature(&self) -> Signature {
        match self {
            Self::Swap(e) => e.signature,
            Self::Liquidity(e) => e.signature,
            Self::ClaimPositionFee(e) => e.signature,
            Self::ClaimReward(e) => e.signature,
            Self::ClaimProtocolFee(e) => e.signature,
            Self::InitializeReward(e) => e.signature,
            Self::FundReward(e) => e.signature,
            Self::WithdrawIneligibleReward(e) => e.signature,
            Self::UpdateRewardDuration(e) => e.signature,
            Self::UpdateRewardFunder(e) => e.signature,
            Self::WithdrawDeadLiquidityReward(e) => e.signature,
            Self::CreatePosition(e) => e.signature,
            Self::ClosePosition(e) => e.signature,
            Self::LockPosition(e) => e.signature,
            Self::PermanentLockPosition(e) => e.signature,
            Self::InitializePool(e) => e.signature,
            Self::SetPoolStatus(e) => e.signature,
            Self::UpdatePoolFees(e) => e.signature,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Swap(e) => e.timestamp,
            Self::Liquidity(e) => e.timestamp,
            Self::ClaimPositionFee(e) => e.timestamp,
            Self::ClaimReward(e) => e.timestamp,
            Self::ClaimProtocolFee(e) => e.timestamp,
            Self::InitializeReward(e) => e.timestamp,
            Self::FundReward(e) => e.timestamp,
            Self::WithdrawIneligibleReward(e) => e.timestamp,
            Self::UpdateRewardDuration(e) => e.timestamp,
            Self::UpdateRewardFunder(e) => e.timestamp,
            Self::WithdrawDeadLiquidityReward(e) => e.timestamp,
            Self::CreatePosition(e) => e.timestamp,
            Self::ClosePosition(e) => e.timestamp,
            Self::LockPosition(e) => e.timestamp,
            Self::PermanentLockPosition(e) => e.timestamp,
            Self::InitializePool(e) => e.timestamp,
            Self::SetPoolStatus(e) => e.timestamp,
            Self::UpdatePoolFees(e) => e.timestamp,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Swap(_) => "swap",
            Self::Liquidity(_) => "liquidity",
            Self::ClaimPositionFee(_) => "claim_position_fee",
            Self::ClaimReward(_) => "claim_reward",
            Self::ClaimProtocolFee(_) => "claim_protocol_fee",
            Self::InitializeReward(_) => "initialize_reward",
            Self::FundReward(_) => "fund_reward",
            Self::WithdrawIneligibleReward(_) => "withdraw_ineligible_reward",
            Self::UpdateRewardDuration(_) => "update_reward_duration",
            Self::UpdateRewardFunder(_) => "update_reward_funder",
            Self::WithdrawDeadLiquidityReward(_) => "withdraw_dead_liquidity_reward",
            Self::CreatePosition(_) => "create_position",
            Self::ClosePosition(_) => "close_position",
            Self::LockPosition(_) => "lock_position",
            Self::PermanentLockPosition(_) => "permanent_lock_position",
            Self::InitializePool(_) => "initialize_pool",
            Self::SetPoolStatus(_) => "set_pool_status",
            Self::UpdatePoolFees(_) => "update_pool_fees",
        }
    }
}
