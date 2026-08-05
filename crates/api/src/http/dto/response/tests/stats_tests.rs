//! Serialisation tests for `StatsResponse`.
//!
//! Found missing in review: `PoolResponse` gained a wire test with the coverage
//! counters, `/api/stats` did not. Its shape was pinned only by a hand-written
//! zod fixture on the web side, which can drift from the API without anything
//! going red — and a coverage figure that silently stops being emitted is worse
//! than one that was never added.

use super::StatsResponse;
use crate::application::StatsAggregate;
use rust_decimal::Decimal;
use yog_core::domain::{GlobalAnalytics, PoolCounts};

fn aggregate(swap_buckets_24h: i64, swap_buckets_priced_24h: i64) -> StatsAggregate {
    StatsAggregate {
        analytics: GlobalAnalytics {
            total_tvl_usd: Some(Decimal::new(10_427_935, 0)),
            pools_priced: 348,
            volume_24h_usd: Some(Decimal::new(508_193, 0)),
            fees_24h_usd: Some(Decimal::new(391, 0)),
            swap_buckets_24h,
            swap_buckets_priced_24h,
        },
        counts: PoolCounts {
            observed: 359,
            discovered_24h: 52,
        },
    }
}

#[test]
fn stats_response_serialises_the_coverage_counters_in_camel_case() {
    let json = serde_json::to_value(StatsResponse::from(aggregate(1246, 767)))
        .expect("StatsResponse must serialise");

    assert_eq!(json["swapBuckets24h"], 1246);
    assert_eq!(json["swapBucketsPriced24h"], 767);
    // The TVL pair it is modelled on must still be there — the two coverages
    // are read side by side on the Overview.
    assert_eq!(json["poolsPriced"], 348);
    assert_eq!(json["poolsObserved"], 359);
}

#[test]
fn a_partial_volume_coverage_survives_the_wire_next_to_its_value() {
    // The payload the pair exists for: a non-null protocol-wide volume covering
    // 767 of the 1246 pool-hours that traded. Nothing in the value says so.
    let json = serde_json::to_value(StatsResponse::from(aggregate(1246, 767))).unwrap();

    assert_eq!(json["volume24hUsd"], serde_json::json!("508193"));
    assert!(
        json["swapBucketsPriced24h"].as_i64().unwrap() < json["swapBuckets24h"].as_i64().unwrap(),
        "a sub-total must be readable as one"
    );
}

#[test]
fn an_empty_universe_reports_zero_coverage_not_full_coverage() {
    let json = serde_json::to_value(StatsResponse::from(aggregate(0, 0))).unwrap();

    assert_eq!(json["swapBuckets24h"], 0);
    assert_eq!(json["swapBucketsPriced24h"], 0);
}
