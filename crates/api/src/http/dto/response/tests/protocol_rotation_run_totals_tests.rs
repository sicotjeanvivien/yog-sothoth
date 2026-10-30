use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{Protocol, RotationRunTotals};

use super::ProtocolRotationRunTotalsResponse;

/// Mutation: the first hour left out of the row, or read from the volume — over the last hour, a
/// field the contract declares missing, or a share at 100 % by construction.
#[test]
fn the_first_hour_is_null_when_not_served() {
    let totals = RotationRunTotals {
        runs: 3,
        pools: 2,
        in_progress: 1,
        median_duration_seconds: Some(300),
        p90_duration_seconds: Some(1200),
        volume_usd: Decimal::new(500, 0),
        first_hour_volume_usd: None,
    };

    let wire = serde_json::to_value(ProtocolRotationRunTotalsResponse::new(
        Protocol::MeteoraDammV2,
        totals,
    ))
    .unwrap();

    assert_eq!(
        wire,
        json!({
            "protocol": "meteora_damm_v2", "runs": 3, "pools": 2, "inProgress": 1,
            "medianDurationSeconds": 300, "p90DurationSeconds": 1200,
            "volumeUsd": "500", "firstHourVolumeUsd": null
        })
    );
}
