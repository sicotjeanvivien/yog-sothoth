use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use solana_pubkey::Pubkey;
use yog_core::domain::{Protocol, RotationRun};

use super::RotationRunResponse;

/// Mutation: two fields swapped in the conversion — a run served with another's volume or end.
#[test]
fn a_run_says_every_field() {
    let run = RotationRun {
        protocol: Protocol::MeteoraDammV2,
        pool_address: Pubkey::new_from_array([1; 32]),
        started_at: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        ended_at: Utc.with_ymd_and_hms(2026, 10, 25, 12, 45, 0).unwrap(),
        volume_usd: Decimal::new(12_345, 1),
        first_hour_volume_usd: Decimal::new(10_000, 1),
        extinct: true,
    };

    let wire = serde_json::to_value(RotationRunResponse::from(run)).unwrap();

    assert_eq!(
        wire,
        json!({
            "protocol": "meteora_damm_v2",
            "poolAddress": Pubkey::new_from_array([1; 32]).to_string(),
            "startedAt": "2026-10-25T12:00:00Z", "endedAt": "2026-10-25T12:45:00Z",
            "volumeUsd": "1234.5", "firstHourVolumeUsd": "1000.0", "extinct": true
        })
    );
}
