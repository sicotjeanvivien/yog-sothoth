mod claim_position_fee_event;
mod claim_reward_event;
mod close_position_event;
mod create_position_event;
mod liquidity_event;
mod lock_position_event;
mod permanent_lock_position_event;
mod swap_event;

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

pub use claim_position_fee_event::{
    MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
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
pub use liquidity_event::{
    MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventCursor,
    MeteoraDammV2LiquidityEventKind, MeteoraDammV2LiquidityEventRepository,
};
pub use lock_position_event::{
    MeteoraDammV2LockPositionEvent, MeteoraDammV2LockPositionEventRepository,
};
pub use permanent_lock_position_event::{
    MeteoraDammV2PermanentLockPositionEvent, MeteoraDammV2PermanentLockPositionEventRepository,
};
pub use swap_event::{
    MeteoraDammV2SwapEvent, MeteoraDammV2SwapEventCursor, MeteoraDammV2SwapEventRepository,
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
    CreatePosition(MeteoraDammV2CreatePositionEvent),
    ClosePosition(MeteoraDammV2ClosePositionEvent),
    LockPosition(MeteoraDammV2LockPositionEvent),
    PermanentLockPosition(MeteoraDammV2PermanentLockPositionEvent),
}

impl MeteoraDammV2Event {
    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::Swap(e) => e.pool_address,
            Self::Liquidity(e) => e.pool_address,
            Self::ClaimPositionFee(e) => e.pool_address,
            Self::ClaimReward(e) => e.pool_address,
            Self::CreatePosition(e) => e.pool_address,
            Self::ClosePosition(e) => e.pool_address,
            Self::LockPosition(e) => e.pool_address,
            Self::PermanentLockPosition(e) => e.pool_address,
        }
    }

    pub fn signature(&self) -> Signature {
        match self {
            Self::Swap(e) => e.signature,
            Self::Liquidity(e) => e.signature,
            Self::ClaimPositionFee(e) => e.signature,
            Self::ClaimReward(e) => e.signature,
            Self::CreatePosition(e) => e.signature,
            Self::ClosePosition(e) => e.signature,
            Self::LockPosition(e) => e.signature,
            Self::PermanentLockPosition(e) => e.signature,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Swap(e) => e.timestamp,
            Self::Liquidity(e) => e.timestamp,
            Self::ClaimPositionFee(e) => e.timestamp,
            Self::ClaimReward(e) => e.timestamp,
            Self::CreatePosition(e) => e.timestamp,
            Self::ClosePosition(e) => e.timestamp,
            Self::LockPosition(e) => e.timestamp,
            Self::PermanentLockPosition(e) => e.timestamp,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Swap(_) => "swap",
            Self::Liquidity(_) => "liquidity",
            Self::ClaimPositionFee(_) => "claim_position_fee",
            Self::ClaimReward(_) => "claim_reward",
            Self::CreatePosition(_) => "create_position",
            Self::ClosePosition(_) => "close_position",
            Self::LockPosition(_) => "lock_position",
            Self::PermanentLockPosition(_) => "permanent_lock_position",
        }
    }
}
