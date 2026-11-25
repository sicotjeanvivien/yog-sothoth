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
        meteora_damm_v2: None,
    };

    let json = serde_json::to_value(ProtocolTvlResponse::from(reading)).unwrap();

    assert_eq!(json["poolsPriced"], 3);
    assert_eq!(json["poolsUnvalued"], 2);
    assert_eq!(
        json["tvlUsd"],
        serde_json::to_value(Decimal::from(100)).unwrap()
    );
}

/// Mutation: the block dropped from the wire, or written as zeros before the measure.
#[test]
fn the_locked_block_is_null_before_the_measure_and_named_after_it() {
    let before = ProtocolTvl {
        protocol: Protocol::MeteoraDammV2,
        at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        tvl_usd: Some(Decimal::from(100)),
        pools_priced: 3,
        pools_unvalued: 2,
        meteora_damm_v2: None,
    };
    let measured = ProtocolTvl {
        meteora_damm_v2: Some(yog_core::domain::ProtocolTvlMeteoraDammV2 {
            locked_usd: Decimal::from(40),
            permanent_usd: Decimal::from(30),
            vesting_usd: Decimal::from(10),
            pools_locked: 5,
            pools_locked_unvalued: 1,
            vesting_covered_since: Some(Utc.timestamp_opt(1_600_000_000, 0).unwrap()),
        }),
        ..before.clone()
    };

    let json = serde_json::to_value(ProtocolTvlResponse::from(before)).unwrap();
    assert!(json["meteoraDammV2"].is_null(), "{json}");

    let json = serde_json::to_value(ProtocolTvlResponse::from(measured)).unwrap();
    let block = &json["meteoraDammV2"];
    assert_eq!(
        block["lockedUsd"],
        serde_json::to_value(Decimal::from(40)).unwrap()
    );
    assert_eq!(
        block["permanentUsd"],
        serde_json::to_value(Decimal::from(30)).unwrap()
    );
    assert_eq!(
        block["vestingUsd"],
        serde_json::to_value(Decimal::from(10)).unwrap()
    );
    assert_eq!(
        (
            block["poolsLocked"].clone(),
            block["poolsLockedUnvalued"].clone()
        ),
        (5.into(), 1.into())
    );
    assert_eq!(block["vestingCoveredSince"], "2020-09-13T12:26:40Z");
}
