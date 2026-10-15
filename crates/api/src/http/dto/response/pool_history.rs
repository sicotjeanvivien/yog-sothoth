//! Wire shape for one hourly bucket of a pool's activity history.
//!
//! Mirrors [`yog_core::domain::PoolHistoryBucket`], with one
//! presentation-derived field (`effectiveFeeBps`) computed by the **same
//! function** as on `PoolResponse` — [`pool::effective_fee_bps`], which this
//! module used to duplicate inline. Returned as a plain ordered array (oldest →
//! newest) by `GET /api/pools/{address}/history` — chart-ready, no pagination
//! (the window is bounded by `days`).
//!
//! `lpFeesUsd` used to be derived here too, as `fees - protocol`. It is now
//! read from the domain type: that formula credited the referral to the LPs,
//! and it was written twice — here and on `PoolResponse`. The split is defined
//! once, in `meteora_damm_v2_pool_hourly_activity` (migration 007).
//!
//! [`pool::effective_fee_bps`]: crate::http::dto::response::pool::effective_fee_bps

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolHistoryBucket;

use super::pool::effective_fee_bps;
use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolHistoryBucketResponse {
    bucket: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    protocol_fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    referral_fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    lp_fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    effective_fee_bps: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    liquidity_added_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    liquidity_removed_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    fees_claimed_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    rewards_claimed_usd: Option<Decimal>,
    #[schema(value_type = Option<Count>)]
    swap_count: Option<i64>,
}

impl From<PoolHistoryBucket> for PoolHistoryBucketResponse {
    fn from(b: PoolHistoryBucket) -> Self {
        Self {
            bucket: b.bucket,
            volume_usd: b.volume_usd,
            fees_usd: b.fees_usd,
            protocol_fees_usd: b.protocol_fees_usd,
            referral_fees_usd: b.referral_fees_usd,
            lp_fees_usd: b.lp_fees_usd,
            effective_fee_bps: effective_fee_bps(b.fees_usd, b.volume_usd),
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
