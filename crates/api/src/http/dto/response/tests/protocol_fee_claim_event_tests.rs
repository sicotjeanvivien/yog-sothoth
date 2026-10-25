use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{MeteoraDammV2ClaimProtocolFeeEvent, ProtocolFeeClaimEvent};

use super::ProtocolFeeClaimEventResponse;

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Mutation: two fields swapped in the conversion — a value served under another name.
#[test]
fn each_field_says_its_own_value() {
    let event = MeteoraDammV2ClaimProtocolFeeEvent {
        pool_address: key(1),
        signature: Signature::from([2; 64]),
        timestamp: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        slot: 9,
        transaction_index: None,
        event_index: 2,
        token_a_amount: 3,
        token_b_amount: 4,
    };

    let wire = serde_json::to_value(ProtocolFeeClaimEventResponse::from(
        ProtocolFeeClaimEvent::MeteoraDammV2(event),
    ))
    .unwrap();

    let k = |seed: u8| key(seed).to_string();
    let own: serde_json::Map<_, _> = ["amountA", "amountB"]
        .into_iter()
        .map(|name| (name.to_owned(), wire[name].clone()))
        .collect();
    assert_eq!(
        serde_json::Value::Object(own),
        json!({"amountA": "3", "amountB": "4"})
    );
    assert_eq!(wire["poolAddress"], k(1));
    assert_eq!(wire["eventIndex"], 2);
}
