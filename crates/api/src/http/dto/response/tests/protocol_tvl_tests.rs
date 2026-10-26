use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{Protocol, ProtocolTvl};

use super::ProtocolTvlResponse;

/// Mutation: the priced or the unvalued pools left off the wire, or swapped.
#[test]
fn a_reading_carries_its_priced_and_unvalued_pools() {
    let reading = ProtocolTvl {
        protocol: Protocol::MeteoraDammV2,
        at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        tvl_usd: Some(Decimal::from(100)),
        pools_priced: 3,
        pools_unvalued: 2,
    };

    let json = serde_json::to_value(ProtocolTvlResponse::from(reading)).unwrap();

    assert_eq!(json["poolsPriced"], 3);
    assert_eq!(json["poolsUnvalued"], 2);
    assert_eq!(
        json["tvlUsd"],
        serde_json::to_value(Decimal::from(100)).unwrap()
    );
}
