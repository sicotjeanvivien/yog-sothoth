use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{PoolSwapBucket, SwapActivity};

use super::PoolSwapBucketResponse;

/// Mutation: the bar's start dropped, or its activity left out of the bar.
#[test]
fn a_pools_bar_says_its_start_and_its_activity() {
    let response = PoolSwapBucketResponse::from(PoolSwapBucket {
        from: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        activity: SwapActivity {
            swaps: 1,
            volume_usd: Some(Decimal::from(2)),
            fees_usd: Some(Decimal::from(3)),
            lp_fees_usd: Some(Decimal::from(4)),
            protocol_fees_usd: Some(Decimal::from(5)),
            referral_fees_usd: Some(Decimal::from(8)),
            pool_hours: 6,
            pool_hours_valued: 7,
        },
    });

    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({
            "from": "2026-10-25T12:00:00Z",
            "swaps": 1,
            "volumeUsd": "2",
            "feesUsd": "3",
            "lpFeesUsd": "4",
            "protocolFeesUsd": "5",
            "referralFeesUsd": "8",
            "poolHours": 6,
            "poolHoursValued": 7,
        })
    );
}
