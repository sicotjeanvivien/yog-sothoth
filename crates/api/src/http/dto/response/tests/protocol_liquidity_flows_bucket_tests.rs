use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{LiquidityFlows, Protocol, ProtocolBucket};

use super::ProtocolLiquidityFlowsBucketResponse;

fn flows() -> LiquidityFlows {
    LiquidityFlows {
        created_usd: Decimal::from(1),
        added_usd: Decimal::from(2),
        removed_usd: Decimal::from(3),
        moves: 4,
        moves_valued: 5,
    }
}

/// Mutation: the sums exchanged in a bar, or its start dropped.
#[test]
fn each_flow_of_a_bucket_is_under_its_name() {
    let bucket = ProtocolLiquidityFlowsBucketResponse::from(ProtocolBucket {
        protocol: Protocol::MeteoraDammV2,
        from: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        value: flows(),
    });

    assert_eq!(
        serde_json::to_value(bucket).unwrap(),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "from": "2026-10-25T12:00:00Z",
            "createdUsd": "1",
            "addedUsd": "2",
            "removedUsd": "3",
            "moves": 4,
            "movesValued": 5,
        })
    );
}
