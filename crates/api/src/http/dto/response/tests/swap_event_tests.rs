use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{MeteoraDammV2SwapEvent, Protocol, SwapEvent, TradeDirection};

use super::SwapEventResponse;

fn event(direction: TradeDirection, fee_token_is_a: bool) -> MeteoraDammV2SwapEvent {
    MeteoraDammV2SwapEvent {
        pool_address: Pubkey::new_from_array([1; 32]),
        signature: Signature::from([2; 64]),
        timestamp: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        slot: 3,
        transaction_index: None,
        event_index: 4,
        trade_direction: direction,
        amount_a: 5,
        amount_b: 6,
        reserve_a_after: 7,
        reserve_b_after: 8,
        next_sqrt_price: 9,
        claiming_fee: 10,
        protocol_fee: 11,
        compounding_fee: 12,
        referral_fee: 13,
        fee_token_is_a,
    }
}

fn wire(event: MeteoraDammV2SwapEvent) -> serde_json::Value {
    serde_json::to_value(SwapEventResponse::from(SwapEvent::MeteoraDammV2(event))).unwrap()
}

/// Mutation: two amounts, reserves or fees exchanged, or a field renamed — a figure published
/// under another's name.
#[test]
fn each_value_of_a_swap_is_under_its_name() {
    assert_eq!(
        wire(event(TradeDirection::AtoB, true)),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "poolAddress": Pubkey::new_from_array([1; 32]).to_string(),
            "signature": Signature::from([2; 64]).to_string(),
            "eventIndex": 4,
            "slot": 3,
            "at": "2026-10-25T12:00:00Z",
            "direction": "a_to_b",
            "amountA": "5",
            "amountB": "6",
            "reserveAAfter": "7",
            "reserveBAfter": "8",
            "meteoraDammV2": {
                "nextSqrtPrice": "9",
                "claimingFee": "10",
                "protocolFee": "11",
                "compoundingFee": "12",
                "referralFee": "13",
                "feeToken": "a",
            },
        })
    );
}

/// Mutation: a direction or a fee token mapped to the other — a sale read as a purchase.
#[test]
fn the_other_direction_and_fee_token_are_said() {
    let other = wire(event(TradeDirection::BtoA, false));

    assert_eq!(other["direction"], "b_to_a");
    assert_eq!(other["meteoraDammV2"]["feeToken"], "b");
}
