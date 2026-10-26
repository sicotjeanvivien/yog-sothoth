use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{
    MeteoraDammV2PoolChange, MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2UpdatePoolFeesEvent,
    PoolChangeEvent,
};

use super::PoolChangeEventResponse;

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

fn block(change: MeteoraDammV2PoolChange) -> serde_json::Value {
    let wire = serde_json::to_value(PoolChangeEventResponse::from(
        PoolChangeEvent::MeteoraDammV2(change),
    ))
    .unwrap();
    assert_eq!(common(&wire), expected_common(Signature::from([2; 64])));
    wire["meteoraDammV2"].clone()
}

fn fee_update(params_raw: Vec<u8>) -> MeteoraDammV2PoolChange {
    MeteoraDammV2PoolChange::Fees(MeteoraDammV2UpdatePoolFeesEvent {
        pool_address: key(1),
        signature: Signature::from([2; 64]),
        timestamp: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        slot: 9,
        transaction_index: None,
        event_index: 2,
        operator: key(10),
        params_raw,
    })
}

/// Mutation: the two variants swapped in the conversion, or the tag renamed — a change read
/// as the other.
#[test]
fn each_change_says_its_kind_and_its_own_fields() {
    let status = block(MeteoraDammV2PoolChange::Status(
        MeteoraDammV2SetPoolStatusEvent {
            pool_address: key(1),
            signature: Signature::from([2; 64]),
            timestamp: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
            slot: 9,
            transaction_index: None,
            event_index: 2,
            status: 7,
        },
    ));
    // A real `params_raw`: the base fee set to 85.999996 %, nothing else.
    let fees = block(fee_update(vec![
        1, 0xd8, 0x8e, 0x42, 0x33, 0, 0, 0, 0, 0, 0,
    ]));

    assert_eq!(status, json!({"kind": "status", "status": 7}));
    assert_eq!(
        fees,
        json!({
            "kind": "fees", "operator": key(10).to_string(),
            "fees": {
                "cliffFeeNumerator": {"change": "set", "value": "859999960"},
                "dynamicFee": {"change": "unchanged"},
                "compoundingFeeBps": {"change": "unchanged"}
            }
        })
    );
}

/// Mutation: `disabled` served as `set`, or an unchanged field served as set.
#[test]
fn a_fee_update_names_each_state_of_its_fields() {
    let mut params_raw = vec![0, 1];
    params_raw.extend([0; 32]);
    params_raw.extend([1, 0xd0, 0x07]);
    assert_eq!(
        block(fee_update(params_raw))["fees"],
        json!({
            "cliffFeeNumerator": {"change": "unchanged"},
            "dynamicFee": {"change": "disabled"},
            "compoundingFeeBps": {"change": "set", "value": 2000}
        })
    );
}

/// Mutation: a blob that does not decode served as anything but `null` — or the item dropped.
#[test]
fn a_fee_update_that_does_not_decode_is_served_with_null_fees() {
    assert_eq!(
        block(fee_update(vec![1, 2, 3])),
        json!({"kind": "fees", "operator": key(10).to_string(), "fees": null})
    );
}
