mod protocol_response;
mod signal_response;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{Pool, PoolAnalytics, SignalRecord};

use super::token::EmbeddedTokenResponse;
use crate::application::EnrichedPool;
use crate::http::dto::schema::{Count, UnsignedDecimal};

pub(super) use protocol_response::ProtocolResponse;
use signal_response::PoolSignalResponse;

/// A pool in API responses. A 24h USD figure skips the hours it cannot value:
/// `swapBucketsPriced24h` of `swapBuckets24h` says how much of the window it
/// covers.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolResponse {
    #[schema(min_length = 1)]
    pool_address: String,
    protocol: ProtocolResponse,
    token_a: EmbeddedTokenResponse,
    token_b: EmbeddedTokenResponse,
    /// Base trading fee in basis points (the genesis tier). `None` while the pool
    /// is unresolved.
    #[schema(value_type = Option<UnsignedDecimal>)]
    fee_bps: Option<Decimal>,
    // Protocol-specific properties belong to `PoolDetailResponse`, not to every protocol's row.
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_24h_usd: Option<Decimal>,
    /// Realized trading fee over the last 24h (USD); its three shares below sum
    /// back to it exactly, and are `None` together with it.
    /// ⚠️ `lpFees24hUsd` is not `fees − protocol`: cp-amm takes the referral out
    /// of the protocol share. All four are read from the analytics.
    #[schema(value_type = Option<UnsignedDecimal>)]
    fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    protocol_fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    referral_fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    lp_fees_24h_usd: Option<Decimal>,
    /// Realized fee rate over the 24h window, in bps (`fees / volume × 10 000`), of
    /// the hours that could be valued; `None` when volume is absent or zero.
    /// ⚠️ Not clipped by the coverage: fees and volume are lost on the same buckets.
    #[schema(value_type = Option<UnsignedDecimal>)]
    effective_fee_bps: Option<Decimal>,
    /// Coverage of the volume and fee figures, not `tvlUsd`: hours of the window
    /// with at least one swap, and how many could be valued. Equal values mean
    /// full coverage.
    #[schema(value_type = Count)]
    swap_buckets_24h: i64,
    #[schema(value_type = Count)]
    swap_buckets_priced_24h: i64,
    /// The pool's signals over the last 24h, newest first, capped per pool. Empty
    /// when the pool was quiet.
    signals_24h: Vec<PoolSignalResponse>,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

/// Realized fee rate in bps, `fees / volume * 10_000`; `None` when volume is unknown or zero.
/// Shared with [`super::pool_history`]: a ratio of two published figures, not a share.
pub(super) fn effective_fee_bps(
    fees_usd: Option<Decimal>,
    volume_usd: Option<Decimal>,
) -> Option<Decimal> {
    match (fees_usd, volume_usd) {
        (Some(fees), Some(volume)) if !volume.is_zero() => {
            Some(fees / volume * Decimal::from(10_000))
        }
        _ => None,
    }
}

impl PoolResponse {
    pub(super) fn new(
        pool: Pool,
        token_a: EmbeddedTokenResponse,
        token_b: EmbeddedTokenResponse,
        analytics: PoolAnalytics,
        recent_signals: Vec<SignalRecord>,
    ) -> Self {
        Self {
            pool_address: pool.pool_address.to_string(),
            protocol: pool.protocol.into(),
            token_a,
            token_b,
            fee_bps: pool.fee_bps,
            tvl_usd: analytics.tvl_usd,
            volume_24h_usd: analytics.volume_24h_usd,
            fees_24h_usd: analytics.fees_24h_usd,
            protocol_fees_24h_usd: analytics.protocol_fees_24h_usd,
            referral_fees_24h_usd: analytics.referral_fees_24h_usd,
            lp_fees_24h_usd: analytics.lp_fees_24h_usd,
            effective_fee_bps: effective_fee_bps(analytics.fees_24h_usd, analytics.volume_24h_usd),
            swap_buckets_24h: analytics.swap_buckets_24h,
            swap_buckets_priced_24h: analytics.swap_buckets_priced_24h,
            signals_24h: recent_signals
                .into_iter()
                .map(PoolSignalResponse::from)
                .collect(),
            first_seen_at: pool.first_seen_at,
            last_seen_at: pool.last_seen_at,
        }
    }
}

impl From<EnrichedPool> for PoolResponse {
    fn from(e: EnrichedPool) -> Self {
        PoolResponse::new(
            e.pool,
            e.token_a.into(),
            e.token_b.into(),
            e.analytics,
            e.recent_signals,
        )
    }
}

#[cfg(test)]
#[path = "tests/pool_tests.rs"]
mod tests;
