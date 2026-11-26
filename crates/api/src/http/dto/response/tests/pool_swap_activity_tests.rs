use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{MeteoraDammV2LpFeeSplit, SwapActivity, SwapActivityPart};

use super::PoolSwapActivityResponse;

/// Mutation: the block left out of a pool's totals, the only pool row not built on a bucket.
#[test]
fn a_damm_v2_pools_totals_carry_its_split_beside_its_fields() {
    let body = serde_json::to_value(PoolSwapActivityResponse::from(SwapActivity {
        swaps: 1,
        volume_usd: None,
        fees_usd: None,
        lp_fees_usd: Some(Decimal::from(5)),
        protocol_fees_usd: None,
        referral_fees_usd: None,
        pool_hours: 1,
        pool_hours_valued: 1,
        protocol_part: Some(SwapActivityPart::MeteoraDammV2(MeteoraDammV2LpFeeSplit {
            claiming_fees_usd: Some(Decimal::from(2)),
            compounding_fees_usd: Some(Decimal::from(3)),
        })),
    }))
    .unwrap();

    assert_eq!(body["lpFeesUsd"], json!("5"));
    assert_eq!(
        body["meteoraDammV2"],
        json!({ "claimingFeesUsd": "2", "compoundingFeesUsd": "3" })
    );
}
