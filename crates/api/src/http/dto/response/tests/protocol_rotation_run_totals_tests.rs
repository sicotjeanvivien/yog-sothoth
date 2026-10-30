use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{Protocol, RotationRunTotals, RunsFirstHour};

use super::ProtocolRotationRunTotalsResponse;

fn wire(first_hour: Option<RunsFirstHour>) -> serde_json::Value {
    let totals = RotationRunTotals {
        runs: 3,
        pools: 2,
        in_progress: 1,
        median_duration_seconds: Some(300),
        p90_duration_seconds: Some(1200),
        volume_usd: Decimal::new(500, 0),
        first_hour,
    };
    serde_json::to_value(ProtocolRotationRunTotalsResponse::new(
        Protocol::MeteoraDammV2,
        totals,
    ))
    .unwrap()
}

/// Mutation: two fields of the block swapped, or the block left out when `None` — a share read
/// on another base, or a field the contract declares missing.
#[test]
fn the_first_hour_is_a_block_or_null() {
    let block = wire(Some(RunsFirstHour {
        runs: 2,
        volume_usd: Decimal::new(400, 0),
        in_first_hour_usd: Decimal::new(390, 0),
    }));
    let none = wire(None);

    assert_eq!(
        block["firstHour"],
        json!({ "runs": 2, "volumeUsd": "400", "inFirstHourUsd": "390" })
    );
    assert_eq!(
        none,
        json!({
            "protocol": "meteora_damm_v2", "runs": 3, "pools": 2, "inProgress": 1,
            "medianDurationSeconds": 300, "p90DurationSeconds": 1200,
            "volumeUsd": "500", "firstHour": null
        })
    );
}
