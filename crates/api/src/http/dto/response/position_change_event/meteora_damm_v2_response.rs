use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{
    MeteoraDammV2LockPositionEvent, MeteoraDammV2PermanentLockPositionEvent,
    MeteoraDammV2PositionChange, MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators,
    MeteoraDammV2SplitPositionEvent, MeteoraDammV2SplitPositionState,
};

use crate::http::dto::schema::{Count, U128String};

/// What a DAMM v2 position change is, named by `kind`, and what that change says.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum PositionChangeEventMeteoraDammV2Response {
    Open(MeteoraDammV2PositionNftResponse),
    Close(MeteoraDammV2PositionNftResponse),
    VestingLock(MeteoraDammV2VestingLockResponse),
    PermanentLock(MeteoraDammV2PermanentLockResponse),
    Split(Box<MeteoraDammV2SplitResponse>),
}

/// The position opened or closed, its owner, and the mint of the NFT that holds it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2PositionNftResponse {
    #[schema(min_length = 1)]
    position: String,
    #[schema(min_length = 1)]
    owner: String,
    #[schema(min_length = 1)]
    position_nft_mint: String,
}

/// The position's liquidity put under a vesting: released at the cliff, then by period.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2VestingLockResponse {
    #[schema(min_length = 1)]
    position: String,
    #[schema(min_length = 1)]
    owner: String,
    #[schema(min_length = 1)]
    vesting: String,
    #[schema(value_type = Count)]
    cliff_point: u64,
    #[schema(value_type = Count)]
    period_frequency: u64,
    #[schema(value_type = U128String)]
    cliff_unlock_liquidity: String,
    #[schema(value_type = U128String)]
    liquidity_per_period: String,
    number_of_period: u16,
}

/// The position's liquidity locked for good, and the position's total so locked after it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2PermanentLockResponse {
    #[schema(min_length = 1)]
    position: String,
    #[schema(value_type = U128String)]
    locked_liquidity: String,
    #[schema(value_type = U128String)]
    total_permanent_locked_liquidity: String,
}

/// A position split in two: what moved from the first to the second, both after it, and the
/// numerators of the shares moved.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2SplitResponse {
    #[schema(min_length = 1)]
    first_position: String,
    #[schema(min_length = 1)]
    second_position: String,
    #[schema(min_length = 1)]
    first_owner: String,
    #[schema(min_length = 1)]
    second_owner: String,
    #[schema(value_type = U128String)]
    current_sqrt_price: String,
    amounts: MeteoraDammV2SplitAmountsResponse,
    first_position_after: MeteoraDammV2SplitStateResponse,
    second_position_after: MeteoraDammV2SplitStateResponse,
    numerators: MeteoraDammV2SplitNumeratorsResponse,
}

/// What a split moved to the second position.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2SplitAmountsResponse {
    #[schema(value_type = U128String)]
    permanent_locked_liquidity: String,
    #[schema(value_type = U128String)]
    unlocked_liquidity: String,
    #[schema(value_type = U128String)]
    vested_liquidity: String,
    #[schema(value_type = U128String)]
    fee_a: String,
    #[schema(value_type = U128String)]
    fee_b: String,
    #[schema(value_type = U128String)]
    reward_0: String,
    #[schema(value_type = U128String)]
    reward_1: String,
}

/// A position after the split.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2SplitStateResponse {
    #[schema(value_type = U128String)]
    unlocked_liquidity: String,
    #[schema(value_type = U128String)]
    permanent_locked_liquidity: String,
    #[schema(value_type = U128String)]
    vested_liquidity: String,
    #[schema(value_type = U128String)]
    fee_a: String,
    #[schema(value_type = U128String)]
    fee_b: String,
    #[schema(value_type = U128String)]
    reward_0: String,
    #[schema(value_type = U128String)]
    reward_1: String,
}

/// The numerators of the shares a split moved.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2SplitNumeratorsResponse {
    unlocked_liquidity: u32,
    permanent_locked_liquidity: u32,
    fee_a: u32,
    fee_b: u32,
    reward_0: u32,
    reward_1: u32,
    inner_vesting_liquidity: u32,
}

impl From<MeteoraDammV2PositionChange> for PositionChangeEventMeteoraDammV2Response {
    fn from(change: MeteoraDammV2PositionChange) -> Self {
        match change {
            MeteoraDammV2PositionChange::Open(e) => Self::Open(MeteoraDammV2PositionNftResponse {
                position: e.position.to_string(),
                owner: e.owner.to_string(),
                position_nft_mint: e.position_nft_mint.to_string(),
            }),
            MeteoraDammV2PositionChange::Close(e) => {
                Self::Close(MeteoraDammV2PositionNftResponse {
                    position: e.position.to_string(),
                    owner: e.owner.to_string(),
                    position_nft_mint: e.position_nft_mint.to_string(),
                })
            }
            MeteoraDammV2PositionChange::VestingLock(e) => Self::VestingLock(e.into()),
            MeteoraDammV2PositionChange::PermanentLock(e) => Self::PermanentLock(e.into()),
            MeteoraDammV2PositionChange::Split(e) => Self::Split(Box::new((*e).into())),
        }
    }
}

impl From<MeteoraDammV2LockPositionEvent> for MeteoraDammV2VestingLockResponse {
    fn from(e: MeteoraDammV2LockPositionEvent) -> Self {
        Self {
            position: e.position.to_string(),
            owner: e.owner.to_string(),
            vesting: e.vesting.to_string(),
            cliff_point: e.cliff_point,
            period_frequency: e.period_frequency,
            cliff_unlock_liquidity: e.cliff_unlock_liquidity.to_string(),
            liquidity_per_period: e.liquidity_per_period.to_string(),
            number_of_period: e.number_of_period,
        }
    }
}

impl From<MeteoraDammV2PermanentLockPositionEvent> for MeteoraDammV2PermanentLockResponse {
    fn from(e: MeteoraDammV2PermanentLockPositionEvent) -> Self {
        Self {
            position: e.position.to_string(),
            locked_liquidity: e.lock_liquidity_amount.to_string(),
            total_permanent_locked_liquidity: e.total_permanent_locked_liquidity.to_string(),
        }
    }
}

impl From<MeteoraDammV2SplitPositionEvent> for MeteoraDammV2SplitResponse {
    fn from(e: MeteoraDammV2SplitPositionEvent) -> Self {
        Self {
            first_position: e.first_position.to_string(),
            second_position: e.second_position.to_string(),
            first_owner: e.first_owner.to_string(),
            second_owner: e.second_owner.to_string(),
            current_sqrt_price: e.current_sqrt_price.to_string(),
            amounts: e.amounts.into(),
            first_position_after: e.first_position_after.into(),
            second_position_after: e.second_position_after.into(),
            numerators: e.numerators.into(),
        }
    }
}

impl From<MeteoraDammV2SplitAmounts> for MeteoraDammV2SplitAmountsResponse {
    fn from(a: MeteoraDammV2SplitAmounts) -> Self {
        Self {
            permanent_locked_liquidity: a.permanent_locked_liquidity.to_string(),
            unlocked_liquidity: a.unlocked_liquidity.to_string(),
            vested_liquidity: a.vested_liquidity.to_string(),
            fee_a: a.fee_a.to_string(),
            fee_b: a.fee_b.to_string(),
            reward_0: a.reward_0.to_string(),
            reward_1: a.reward_1.to_string(),
        }
    }
}

impl From<MeteoraDammV2SplitPositionState> for MeteoraDammV2SplitStateResponse {
    fn from(s: MeteoraDammV2SplitPositionState) -> Self {
        Self {
            unlocked_liquidity: s.unlocked_liquidity.to_string(),
            permanent_locked_liquidity: s.permanent_locked_liquidity.to_string(),
            vested_liquidity: s.vested_liquidity.to_string(),
            fee_a: s.fee_a.to_string(),
            fee_b: s.fee_b.to_string(),
            reward_0: s.reward_0.to_string(),
            reward_1: s.reward_1.to_string(),
        }
    }
}

impl From<MeteoraDammV2SplitNumerators> for MeteoraDammV2SplitNumeratorsResponse {
    fn from(n: MeteoraDammV2SplitNumerators) -> Self {
        Self {
            unlocked_liquidity: n.unlocked_liquidity,
            permanent_locked_liquidity: n.permanent_locked_liquidity,
            fee_a: n.fee_a,
            fee_b: n.fee_b,
            reward_0: n.reward_0,
            reward_1: n.reward_1,
            inner_vesting_liquidity: n.inner_vesting_liquidity,
        }
    }
}
