use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{MeteoraDammV2LpFeeSplit, PoolSwapBucket, SwapActivity};

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
            meteora_damm_v2: None,
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

/// Mutation: the block left out of a DAMM v2 pool's bar, or an unmeasured part omitted, not `null`.
#[test]
fn a_damm_v2_pools_bar_carries_its_split() {
    let response = PoolSwapBucketResponse::from(PoolSwapBucket {
        from: Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap(),
        activity: SwapActivity {
            swaps: 0,
            volume_usd: None,
            fees_usd: None,
            lp_fees_usd: Some(Decimal::from(4)),
            protocol_fees_usd: None,
            referral_fees_usd: None,
            pool_hours: 0,
            pool_hours_valued: 0,
            meteora_damm_v2: Some(MeteoraDammV2LpFeeSplit {
                claiming_fees_usd: Some(Decimal::from(3)),
                compounding_fees_usd: None,
            }),
        },
    });

    assert_eq!(
        serde_json::to_value(response).unwrap()["meteoraDammV2"],
        json!({ "claimingFeesUsd": "3", "compoundingFeesUsd": null })
    );
}
