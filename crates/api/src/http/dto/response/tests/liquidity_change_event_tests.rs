use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{
    LiquidityChangeEvent, MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventKind, Protocol,
};

use super::LiquidityChangeEventResponse;

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn wire(kind: MeteoraDammV2LiquidityEventKind) -> serde_json::Value {
    let event = MeteoraDammV2LiquidityEvent {
        pool_address: key(1),
        signature: Signature::from([2; 64]),
        timestamp: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        slot: 3,
        transaction_index: None,
        event_index: 4,
        liquidity_event_kind: kind,
        amount_a: 5,
        amount_b: 6,
        liquidity_delta: 7,
        reserve_a_after: 8,
        reserve_b_after: 9,
        position: key(10),
        owner: key(11),
    };
    serde_json::to_value(LiquidityChangeEventResponse::from(
        LiquidityChangeEvent::MeteoraDammV2(event),
    ))
    .unwrap()
}

/// Mutation: two amounts or reserves exchanged, the position written as the owner, or a field
/// renamed — a value published under another's name.
#[test]
fn each_value_of_a_change_is_under_its_name() {
    assert_eq!(
        wire(MeteoraDammV2LiquidityEventKind::Add),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "poolAddress": key(1).to_string(),
            "signature": Signature::from([2; 64]).to_string(),
            "eventIndex": 4,
            "slot": 3,
            "at": "2026-10-25T12:00:00Z",
            "kind": "add",
            "amountA": "5",
            "amountB": "6",
            "reserveAAfter": "8",
            "reserveBAfter": "9",
            "position": key(10).to_string(),
            "owner": key(11).to_string(),
            "meteoraDammV2": { "liquidityDelta": "7" },
        })
    );
}

/// Mutation: the kinds mapped to each other — a withdrawal read as a deposit.
#[test]
fn a_removal_is_said() {
    assert_eq!(
        wire(MeteoraDammV2LiquidityEventKind::Remove)["kind"],
        "remove"
    );
}
