use chrono::{TimeZone, Utc};
use serde_json::json;
use yog_core::domain::{PoolCreations, Protocol, ProtocolBucket};

use super::ProtocolPoolCreationsBucketResponse;

/// Mutation: a creations' bar without its count or its start.
#[test]
fn a_bucket_of_creations_says_its_pools() {
    let bucket = ProtocolPoolCreationsBucketResponse::from(ProtocolBucket {
        protocol: Protocol::MeteoraDammV2,
        from: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        value: PoolCreations { pools: 7 },
    });

    assert_eq!(
        serde_json::to_value(bucket).unwrap(),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "from": "2026-10-25T12:00:00Z",
            "pools": 7,
        })
    );
}
