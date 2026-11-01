use chrono::{TimeZone, Utc};
use serde_json::json;
use yog_core::domain::{PositionChangeCounts, Protocol, ProtocolBucket};

use super::{ProtocolPositionChangeBucketResponse, ProtocolPositionChangeTotalsResponse};

fn counts() -> PositionChangeCounts {
    PositionChangeCounts {
        opened: 1,
        vesting_locks: 2,
        permanent_locks: 3,
        splits: 4,
        closed: 5,
    }
}

/// Mutation: two counts exchanged, or the bucket's start dropped — the openings read as closures.
#[test]
fn each_count_is_under_its_name() {
    let expected = json!({
        "protocol": Protocol::MeteoraDammV2.as_str(),
        "opened": 1,
        "vestingLocks": 2,
        "permanentLocks": 3,
        "splits": 4,
        "closed": 5,
    });

    let total = ProtocolPositionChangeTotalsResponse::new(Protocol::MeteoraDammV2, counts());
    assert_eq!(serde_json::to_value(total).unwrap(), expected);

    let bucket = ProtocolPositionChangeBucketResponse::from(ProtocolBucket {
        protocol: Protocol::MeteoraDammV2,
        from: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        value: counts(),
    });
    let mut with_from = expected;
    with_from["from"] = json!("2026-10-25T12:00:00Z");
    assert_eq!(serde_json::to_value(bucket).unwrap(), with_from);
}
