//! Unit tests for `From<PoolHistoryBucket> for PoolHistoryBucketResponse` —
//! the one derived field, and the fee shares that must NOT be derived.

use chrono::Utc;
use rust_decimal::Decimal;
use yog_core::domain::PoolHistoryBucket;

use crate::http::dto::response::pool_history::PoolHistoryBucketResponse;

/// A bucket whose referral is non-zero, on purpose: it makes
/// `fees - protocol` (20) differ from the true LP share (19), so the
/// tests below can tell a pass-through from the old subtraction.
fn bucket() -> PoolHistoryBucket {
    PoolHistoryBucket {
        bucket: Utc::now(),
        volume_usd: Some(Decimal::new(10_000, 0)),
        fees_usd: Some(Decimal::new(25, 0)),
        protocol_fees_usd: Some(Decimal::new(5, 0)),
        referral_fees_usd: Some(Decimal::new(1, 0)),
        lp_fees_usd: Some(Decimal::new(19, 0)),
        liquidity_added_usd: None,
        liquidity_removed_usd: None,
        fees_claimed_usd: None,
        rewards_claimed_usd: None,
        swap_count: Some(3),
    }
}

#[test]
fn fee_shares_are_carried_through_not_recomputed() {
    let resp = PoolHistoryBucketResponse::from(bucket());

    assert_eq!(resp.protocol_fees_usd, Some(Decimal::new(5, 0)));
    assert_eq!(resp.referral_fees_usd, Some(Decimal::new(1, 0)));
    // 19, not 20: `fees - protocol` would credit the referral to the LPs.
    // This is the assertion that fails if the subtraction comes back.
    assert_eq!(resp.lp_fees_usd, Some(Decimal::new(19, 0)));
}

#[test]
fn derives_effective_rate() {
    let resp = PoolHistoryBucketResponse::from(bucket());
    // effective = fees / volume * 10000 = 25 / 10000 * 10000 = 25 bps
    assert_eq!(resp.effective_fee_bps, Some(Decimal::new(25, 0)));
}

#[test]
fn effective_rate_none_when_inputs_missing_or_zero() {
    let resp = PoolHistoryBucketResponse::from(PoolHistoryBucket {
        fees_usd: None,
        volume_usd: Some(Decimal::ZERO),
        ..bucket()
    });
    assert_eq!(resp.effective_fee_bps, None);
}

#[test]
fn absent_fee_shares_stay_absent() {
    // The view makes the four NULL together; the mapping must not invent a
    // value for one of them.
    let resp = PoolHistoryBucketResponse::from(PoolHistoryBucket {
        fees_usd: None,
        protocol_fees_usd: None,
        referral_fees_usd: None,
        lp_fees_usd: None,
        ..bucket()
    });
    assert_eq!(resp.fees_usd, None);
    assert_eq!(resp.protocol_fees_usd, None);
    assert_eq!(resp.referral_fees_usd, None);
    assert_eq!(resp.lp_fees_usd, None);
}
