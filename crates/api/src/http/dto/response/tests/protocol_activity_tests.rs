//! The wire shape of `/api/protocols/tvl`: the TVL, its unvalued pools, and its
//! discovered part for each period, labelled as the windows are.

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{DiscoveredTvl, LatestProtocolTvl, Protocol, ProtocolTvl, Window};

use super::LatestProtocolTvlResponse;

/// Mutations: a discovered part without its period, or with the window's Rust name; the
/// unvalued pools left off the wire.
#[test]
fn the_latest_tvl_carries_its_unvalued_pools_and_its_discovered_parts() {
    let latest = LatestProtocolTvl {
        tvl: ProtocolTvl {
            protocol: Protocol::MeteoraDammV2,
            at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            tvl_usd: Some(Decimal::from(100)),
            pools_priced: 3,
            pools_unvalued: 2,
        },
        discovered: vec![
            DiscoveredTvl {
                window: Window::Day,
                tvl_usd: Decimal::from(5),
                pools: 1,
            },
            DiscoveredTvl {
                window: Window::Week,
                tvl_usd: Decimal::from(7),
                pools: 2,
            },
        ],
    };

    let json = serde_json::to_value(LatestProtocolTvlResponse::from(latest)).unwrap();

    assert_eq!(json["poolsPriced"], 3);
    assert_eq!(json["poolsUnvalued"], 2);
    assert_eq!(
        json["tvlUsd"],
        serde_json::to_value(Decimal::from(100)).unwrap()
    );
    assert_eq!(
        json["discovered"],
        serde_json::json!([
            { "period": "24h", "tvlUsd": Decimal::from(5), "pools": 1 },
            { "period": "7d", "tvlUsd": Decimal::from(7), "pools": 2 },
        ])
    );
}
