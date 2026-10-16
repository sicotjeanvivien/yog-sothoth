//! One hourly bucket of a pool's history, oldest to newest, with `effectiveFeeBps` computed by
//! [`pool::effective_fee_bps`], as on `PoolResponse`.
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
