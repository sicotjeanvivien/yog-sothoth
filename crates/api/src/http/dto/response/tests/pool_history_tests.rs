//! Unit tests for `From<PoolHistoryBucket> for PoolHistoryBucketResponse` —
//! specifically the two derived fields.

use chrono::Utc;
use rust_decimal::Decimal;
use yog_core::domain::PoolHistoryBucket;

use crate::http::dto::response::pool_history::PoolHistoryBucketResponse;

fn bucket() -> PoolHistoryBucket {
    PoolHistoryBucket {
        bucket: Utc::now(),
        volume_usd: Some(Decimal::new(10_000, 0)),
        fees_usd: Some(Decimal::new(25, 0)),
        protocol_fees_usd: Some(Decimal::new(5, 0)),
        liquidity_added_usd: None,
        liquidity_removed_usd: None,
        fees_claimed_usd: None,
        rewards_claimed_usd: None,
        swap_count: Some(3),
    }
}

#[test]
fn derives_lp_fees_and_effective_rate() {
    let resp = PoolHistoryBucketResponse::from(bucket());
    // lp = fees - protocol = 25 - 5 = 20
    assert_eq!(resp.lp_fees_usd, Some(Decimal::new(20, 0)));
    // effective = fees / volume * 10000 = 25 / 10000 * 10000 = 25 bps
    assert_eq!(resp.effective_fee_bps, Some(Decimal::new(25, 0)));
}

#[test]
fn derived_fields_none_when_inputs_missing_or_zero() {
    let resp = PoolHistoryBucketResponse::from(PoolHistoryBucket {
        fees_usd: None,
        volume_usd: Some(Decimal::ZERO),
        protocol_fees_usd: None,
        ..bucket()
    });
    assert_eq!(resp.lp_fees_usd, None);
    assert_eq!(resp.effective_fee_bps, None);
}
