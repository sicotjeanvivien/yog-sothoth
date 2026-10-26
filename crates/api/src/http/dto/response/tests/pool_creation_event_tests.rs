use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{MeteoraDammV2InitializePoolEvent, PoolCreationEvent};

use super::PoolCreationEventResponse;

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The fees block of a creation whose `pool_fees_raw` is `raw`; `None` when the key is absent.
fn fees(raw: Vec<u8>) -> Option<serde_json::Value> {
    let key = |seed| Pubkey::new_from_array([seed; 32]);
    let event = MeteoraDammV2InitializePoolEvent {
        pool_address: key(1),
        signature: Signature::from([2; 64]),
        timestamp: Utc.with_ymd_and_hms(2026, 10, 15, 12, 0, 0).unwrap(),
        slot: 9,
        transaction_index: None,
        event_index: 2,
        token_a_mint: key(3),
        token_b_mint: key(4),
        creator: key(5),
        payer: key(6),
        alpha_vault: key(7),
        sqrt_min_price: 1,
        sqrt_max_price: 2,
        sqrt_price: 1,
        liquidity: 3,
        activation_type: 1,
        activation_point: 4,
        collect_fee_mode: 0,
        pool_type: 0,
        token_a_flag: 0,
        token_b_flag: 0,
        token_a_amount: 5,
        token_b_amount: 6,
        total_amount_a: 5,
        total_amount_b: 6,
        pool_fees_raw: raw,
    };
    let wire = serde_json::to_value(PoolCreationEventResponse::from(
        PoolCreationEvent::MeteoraDammV2(event),
    ))
    .unwrap();
    wire["meteoraDammV2"].get("fees").cloned()
}

/// Mutation: the mode tag renamed, a `u64` served as a number, or `off` and `on` swapped.
/// Real `pool_fees_raw`: 88 % falling over 180 one-second periods, then a mode 3 with a
/// dynamic fee.
#[test]
fn a_creation_serves_its_fees_named_by_their_mode() {
    assert_eq!(
        fees(hex(
            "00bc733400000000b40001000000000000002e0e48000000000000d0070000"
        )),
        Some(json!({
            "baseFee": {
                "mode": "time_scheduler_linear", "cliffFeeNumerator": "880000000",
                "numberOfPeriod": 180, "periodFrequency": "1", "reductionFactor": "4722222"
            },
            "compoundingFeeBps": 2000,
            "dynamicFee": {"state": "off"}
        }))
    );
    assert_eq!(
        fees(hex(
            "0027b929000000001b0083020000008d2700d50964010000000003000000010100cb10c7bab88d060000000000000000000a007800881360a4dc008c050100"
        )),
        Some(json!({
            "baseFee": {
                "mode": "market_cap_scheduler_linear", "cliffFeeNumerator": "700000000",
                "numberOfPeriod": 27, "sqrtPriceStepBps": 643,
                "schedulerExpirationDuration": 2592000, "reductionFactor": "23333333"
            },
            "compoundingFeeBps": 0,
            "dynamicFee": {
                "state": "on", "binStep": 1, "binStepU128": "1844674407370955",
                "filterPeriod": 10, "decayPeriod": 120, "reductionFactor": 5000,
                "maxVolatilityAccumulator": 14460000, "variableFeeControl": 66956
            }
        }))
    );
}

/// Mutation: a blob that does not decode served as anything but `null`, the key omitted included.
#[test]
fn a_creation_whose_fees_do_not_decode_serves_null() {
    assert_eq!(fees(vec![0; 30]), Some(json!(null)));
}
