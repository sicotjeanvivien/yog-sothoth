use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{Protocol, ProtocolDiscoveredTvl};

use super::ProtocolDiscoveredTvlResponse;

/// Mutation: the count and the sum exchanged, or the instant dropped.
#[test]
fn each_value_is_under_its_name() {
    let response = ProtocolDiscoveredTvlResponse::from(ProtocolDiscoveredTvl {
        protocol: Protocol::MeteoraDammV2,
        at: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        pools: 1,
        tvl_usd: Decimal::from(2),
    });

    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "at": "2026-10-25T12:00:00Z",
            "pools": 1,
            "tvlUsd": "2",
        })
    );
}
