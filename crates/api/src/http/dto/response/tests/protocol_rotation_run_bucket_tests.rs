use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{Protocol, ProtocolBucket, RunSplit};

use super::ProtocolRotationRunBucketResponse;

fn wire(volume_usd: Option<Decimal>, in_runs_usd: Option<Decimal>) -> serde_json::Value {
    serde_json::to_value(ProtocolRotationRunBucketResponse::from(ProtocolBucket {
        protocol: Protocol::MeteoraDammV2,
        from: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        value: RunSplit {
            volume_usd,
            in_runs_usd,
        },
    }))
    .unwrap()
}

/// Mutation: the volume and the part in runs exchanged — the bar drawn the wrong way round.
#[test]
fn each_part_of_a_bar_is_under_its_name() {
    assert_eq!(
        wire(Some(Decimal::from(5)), Some(Decimal::from(2))),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "from": "2026-10-25T12:00:00Z",
            "volumeUsd": "5",
            "inRunsUsd": "2",
            "outsideRunsUsd": "3",
        })
    );
}

/// Mutation: an unknown part answered as 0 — a bar not covered drawn as measured.
#[test]
fn an_unknown_part_stays_null() {
    let bar = wire(Some(Decimal::from(5)), None);

    assert_eq!(bar["inRunsUsd"], serde_json::Value::Null);
    assert_eq!(bar["outsideRunsUsd"], serde_json::Value::Null);
}
