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

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// The fields every protocol's item says, the protocol's own block left out.
fn common(wire: &serde_json::Value) -> serde_json::Value {
    let fields = wire.as_object().unwrap();
    serde_json::Value::Object(
        fields
            .iter()
            .filter(|(name, _)| *name != "meteoraDammV2")
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
    )
}

fn expected_common(signature: Signature) -> serde_json::Value {
    json!({
        "protocol": "meteora_damm_v2", "poolAddress": key(1).to_string(),
        "signature": signature.to_string(), "eventIndex": 2, "slot": 9,
        "at": "2026-10-25T12:00:00Z"
    })
}

/// Mutation: two variants or two fields swapped in the conversion, or the tag renamed — a
/// change read as another kind, a field as another, or its kind under another name.
#[test]
fn each_change_says_its_kind_and_its_own_fields() {
    let at = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let signature = Signature::from([1; 64]);
    let open = MeteoraDammV2CreatePositionEvent {
        pool_address: key(1),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        owner: key(10),
        position: key(11),
        position_nft_mint: key(12),
    };
    let close = MeteoraDammV2ClosePositionEvent {
        pool_address: key(1),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        owner: key(13),
        position: key(14),
        position_nft_mint: key(15),
    };
    let lock = MeteoraDammV2LockPositionEvent {
        pool_address: key(1),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        position: key(16),
        owner: key(17),
        vesting: key(18),
        cliff_point: 1,
        period_frequency: 2,
        cliff_unlock_liquidity: 1 << 64,
        liquidity_per_period: 4,
        number_of_period: 5,
    };
    let permanent = MeteoraDammV2PermanentLockPositionEvent {
        pool_address: key(1),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        position: key(19),
        lock_liquidity_amount: 6,
        total_permanent_locked_liquidity: 7,
    };
    let state = |base: u64| MeteoraDammV2SplitPositionState {
        unlocked_liquidity: u128::from(base + 1),
        permanent_locked_liquidity: u128::from(base + 2),
        vested_liquidity: u128::from(base + 3),
        fee_a: base + 4,
        fee_b: base + 5,
        reward_0: base + 6,
        reward_1: base + 7,
    };
    let split = MeteoraDammV2SplitPositionEvent {
        pool_address: key(1),
        signature,
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 2,
        first_owner: key(20),
        second_owner: key(21),
        first_position: key(22),
        second_position: key(23),
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
        first_position_after: state(10),
        second_position_after: state(20),
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

    let k = |seed: u8| key(seed).to_string();
    for change in [&open, &close, &lock, &permanent, &split] {
        assert_eq!(common(change), expected_common(signature));
    }
    assert_eq!(
        open["meteoraDammV2"],
        json!({"kind": "open", "owner": k(10), "position": k(11), "positionNftMint": k(12)})
    );
    assert_eq!(
        close["meteoraDammV2"],
        json!({"kind": "close", "owner": k(13), "position": k(14), "positionNftMint": k(15)})
    );
    assert_eq!(
        lock["meteoraDammV2"],
        json!({
            "kind": "vesting_lock", "position": k(16), "owner": k(17), "vesting": k(18),
            "cliffPoint": "1", "periodFrequency": "2",
            "cliffUnlockLiquidity": "18446744073709551616", "liquidityPerPeriod": "4",
            "numberOfPeriod": 5
        })
    );
    assert_eq!(
        permanent["meteoraDammV2"],
        json!({
            "kind": "permanent_lock", "position": k(19), "lockedLiquidity": "6",
            "totalPermanentLockedLiquidity": "7"
        })
    );
    let state = |base: u64| {
        json!({
            "unlockedLiquidity": (base + 1).to_string(),
            "permanentLockedLiquidity": (base + 2).to_string(),
            "vestedLiquidity": (base + 3).to_string(), "feeA": (base + 4).to_string(),
            "feeB": (base + 5).to_string(), "reward0": (base + 6).to_string(),
            "reward1": (base + 7).to_string()
        })
    };
    assert_eq!(
        split["meteoraDammV2"],
        json!({
            "kind": "split", "firstOwner": k(20), "secondOwner": k(21), "firstPosition": k(22),
            "secondPosition": k(23), "currentSqrtPrice": "8",
            "amounts": {
                "permanentLockedLiquidity": "1", "unlockedLiquidity": "2",
                "vestedLiquidity": "3", "feeA": "4", "feeB": "5", "reward0": "6", "reward1": "7"
            },
            "firstPositionAfter": state(10),
            "secondPositionAfter": state(20),
            "numerators": {
                "unlockedLiquidity": 1, "permanentLockedLiquidity": 2, "feeA": 3, "feeB": 4,
                "reward0": 5, "reward1": 6, "innerVestingLiquidity": 7
            }
        })
    );
}
