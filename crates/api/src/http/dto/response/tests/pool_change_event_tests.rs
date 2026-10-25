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

/// Mutation: the two variants swapped in the conversion, the tag renamed, or the raw fee
/// parameters served — a change read as the other, or bytes no client can read.
#[test]
fn each_change_says_its_kind_and_its_own_fields() {
    let at = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let signature = Signature::from([2; 64]);

    let status = block(MeteoraDammV2PoolChange::Status(
        MeteoraDammV2SetPoolStatusEvent {
            pool_address: key(1),
            signature,
            timestamp: at,
            slot: 9,
            transaction_index: None,
            event_index: 2,
            status: 7,
        },
    ));
    let fees = block(MeteoraDammV2PoolChange::Fees(
        MeteoraDammV2UpdatePoolFeesEvent {
            pool_address: key(1),
            signature,
            timestamp: at,
            slot: 9,
            transaction_index: None,
            event_index: 2,
            operator: key(10),
            params_raw: vec![1, 2, 3],
        },
    ));

    assert_eq!(status, json!({"kind": "status", "status": 7}));
    assert_eq!(
        fees,
        json!({"kind": "fees", "operator": key(10).to_string()})
    );
}
