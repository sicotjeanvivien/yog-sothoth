use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2CreatePositionEvent,
    MeteoraDammV2LockPositionEvent, MeteoraDammV2PermanentLockPositionEvent,
    MeteoraDammV2PositionChange, MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators,
    MeteoraDammV2SplitPositionEvent, MeteoraDammV2SplitPositionState, PositionChangeEvent,
};

use super::PositionChangeEventResponse;

fn wire(change: MeteoraDammV2PositionChange) -> serde_json::Value {
    serde_json::to_value(PositionChangeEventResponse::from(
        PositionChangeEvent::MeteoraDammV2(change),
    ))
    .unwrap()
}

fn key() -> Pubkey {
    Pubkey::new_from_array([7; 32])
}

/// Mutation: two variants swapped in the conversion, or the tag renamed — a change read as
/// another kind, or its kind under another name.
#[test]
fn each_change_says_its_kind_and_its_own_fields() {
    let at = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let signature = Signature::from([1; 64]);
    let open = MeteoraDammV2CreatePositionEvent {
        pool_address: key(),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        owner: key(),
        position: key(),
        position_nft_mint: key(),
    };
    let close = MeteoraDammV2ClosePositionEvent {
        pool_address: key(),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        owner: key(),
        position: key(),
        position_nft_mint: key(),
    };
    let lock = MeteoraDammV2LockPositionEvent {
        pool_address: key(),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        position: key(),
        owner: key(),
        vesting: key(),
        cliff_point: 1,
        period_frequency: 2,
        cliff_unlock_liquidity: 1 << 64,
        liquidity_per_period: 4,
        number_of_period: 5,
    };
    let permanent = MeteoraDammV2PermanentLockPositionEvent {
        pool_address: key(),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        position: key(),
        lock_liquidity_amount: 6,
        total_permanent_locked_liquidity: 7,
    };
    let state = MeteoraDammV2SplitPositionState {
        unlocked_liquidity: 1,
        permanent_locked_liquidity: 2,
        vested_liquidity: 3,
        fee_a: 4,
        fee_b: 5,
        reward_0: 6,
        reward_1: 7,
    };
    let split = MeteoraDammV2SplitPositionEvent {
        pool_address: key(),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        first_owner: key(),
        second_owner: key(),
        first_position: key(),
        second_position: key(),
        current_sqrt_price: 8,
        amounts: MeteoraDammV2SplitAmounts {
            permanent_locked_liquidity: 1,
            unlocked_liquidity: 2,
            vested_liquidity: 3,
            fee_a: 4,
            fee_b: 5,
            reward_0: 6,
            reward_1: 7,
        },
        first_position_after: state,
        second_position_after: state,
        numerators: MeteoraDammV2SplitNumerators {
            unlocked_liquidity: 1,
            permanent_locked_liquidity: 2,
            fee_a: 3,
            fee_b: 4,
            reward_0: 5,
            reward_1: 6,
            inner_vesting_liquidity: 7,
        },
    };

    let open = wire(MeteoraDammV2PositionChange::Open(open));
    let close = wire(MeteoraDammV2PositionChange::Close(close));
    let lock = wire(MeteoraDammV2PositionChange::VestingLock(lock));
    let permanent = wire(MeteoraDammV2PositionChange::PermanentLock(permanent));
    let split = wire(MeteoraDammV2PositionChange::Split(Box::new(split)));

    assert_eq!(open["eventIndex"], 2);
    assert_eq!(open["meteoraDammV2"]["kind"], "open");
    assert_eq!(open["meteoraDammV2"]["positionNftMint"], key().to_string());
    assert_eq!(close["meteoraDammV2"]["kind"], "close");
    assert_eq!(lock["meteoraDammV2"]["kind"], "vesting_lock");
    assert_eq!(
        lock["meteoraDammV2"]["cliffUnlockLiquidity"],
        "18446744073709551616"
    );
    assert_eq!(permanent["meteoraDammV2"]["kind"], "permanent_lock");
    assert_eq!(permanent["meteoraDammV2"]["lockedLiquidity"], "6");
    assert_eq!(split["meteoraDammV2"]["kind"], "split");
    assert_eq!(split["meteoraDammV2"]["amounts"]["reward1"], "7");
    assert_eq!(
        split["meteoraDammV2"]["numerators"],
        json!({
            "unlockedLiquidity": 1, "permanentLockedLiquidity": 2, "feeA": 3, "feeB": 4,
            "reward0": 5, "reward1": 6, "innerVestingLiquidity": 7
        })
    );
}
