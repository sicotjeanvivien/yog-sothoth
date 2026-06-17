//! Wire shape for one hourly bucket of a pool's activity history.
//!
//! Mirrors `yog_core::domain::PoolHistoryBucket`, with two presentation-derived
//! fields (`lpFeesUsd`, `effectiveFeeBps`) computed the same way as on
//! `PoolResponse`. Returned as a plain ordered array (oldest → newest) by
//! `GET /api/pools/{address}/history` — chart-ready, no pagination (the window
//! is bounded by `days`).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

use yog_core::domain::PoolHistoryBucket;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolHistoryBucketResponse {
    pub(crate) bucket: DateTime<Utc>,
    pub(crate) volume_usd: Option<Decimal>,
    pub(crate) fees_usd: Option<Decimal>,
    pub(crate) protocol_fees_usd: Option<Decimal>,
    pub(crate) lp_fees_usd: Option<Decimal>,
    pub(crate) effective_fee_bps: Option<Decimal>,
    pub(crate) liquidity_added_usd: Option<Decimal>,
    pub(crate) liquidity_removed_usd: Option<Decimal>,
    pub(crate) fees_claimed_usd: Option<Decimal>,
    pub(crate) rewards_claimed_usd: Option<Decimal>,
    pub(crate) swap_count: Option<i64>,
}

impl From<PoolHistoryBucket> for PoolHistoryBucketResponse {
    fn from(b: PoolHistoryBucket) -> Self {
        let lp_fees_usd = match (b.fees_usd, b.protocol_fees_usd) {
            (Some(fees), Some(protocol)) => Some(fees - protocol),
            _ => None,
        };
        let effective_fee_bps = match (b.fees_usd, b.volume_usd) {
            (Some(fees), Some(volume)) if !volume.is_zero() => {
                Some(fees / volume * Decimal::from(10_000))
            }
            _ => None,
        };
        Self {
            bucket: b.bucket,
            volume_usd: b.volume_usd,
            fees_usd: b.fees_usd,
            protocol_fees_usd: b.protocol_fees_usd,
            lp_fees_usd,
            effective_fee_bps,
            liquidity_added_usd: b.liquidity_added_usd,
            liquidity_removed_usd: b.liquidity_removed_usd,
            fees_claimed_usd: b.fees_claimed_usd,
            rewards_claimed_usd: b.rewards_claimed_usd,
            swap_count: b.swap_count,
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_history_tests.rs"]
mod tests;
