mod protocol_response;
mod signal_response;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{Pool, PoolAnalytics, SignalRecord};

use crate::http::dto::schema::{Count, UnsignedDecimal};
use crate::{
    application::{EnrichedPool, EnrichedToken},
    http::dto::EmbeddedTokenResponse,
};

pub(super) use protocol_response::ProtocolResponse;
use signal_response::PoolSignalResponse;

/// Wire shape of a pool in API responses.
///
/// Independent from the domain `Pool` so the public contract can evolve
/// (rename `pool_address` → `address`, etc.) without breaking internal
/// representations. Pubkeys are formatted as base58, protocol as
/// snake_case (matching its `Serialize` impl).
///
/// Analytics (TVL, 24h volume) are denominated in USD. They are
/// `Option` because their computation requires data that may not be
/// available yet (no current state, no priced token, no swap in the
/// window). Serialised as JSON **strings** via `rust_decimal`'s default
/// representation, which keeps the trailing digits a JS `number` would drop.
///
/// A non-null 24h figure is not necessarily a *complete* one: the sum skips
/// the hours it cannot value. `swapBuckets24h` / `swapBucketsPriced24h` carry
/// that coverage so a partial total cannot pass for a full one.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolResponse {
    #[schema(min_length = 1)]
    pub(crate) pool_address: String,
    pub(crate) protocol: ProtocolResponse,
    pub(crate) token_a: EmbeddedTokenResponse,
    pub(crate) token_b: EmbeddedTokenResponse,
    /// Base trading fee in basis points (genesis fee tier). `None` until the
    /// pool's `InitializePool` event has been indexed.
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) fee_bps: Option<Decimal>,
    // NOTE: the cp-amm fee properties (fee-split percents, fee shape) are NOT
    // here. They are protocol-specific, and only the pool *detail* sheet shows
    // them — see `PoolDetailResponse`. Keeping them on the list payload would
    // put one protocol's vocabulary in every protocol's row.
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) tvl_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) volume_24h_usd: Option<Decimal>,
    /// Realized trading fee over the last 24h (USD), and its three shares:
    /// Meteora's cut, the referrer's cut, and what is left for the LPs. They
    /// sum back to `fees_24h_usd` exactly, and are `None` together with it,
    /// under the same partial-price coverage rules as `volume_24h_usd`.
    ///
    /// All four are read straight from the analytics — the split is computed
    /// once in SQL. `lp_fees_24h_usd` is **not** `fees - protocol`: cp-amm
    /// takes the referral out of the protocol share, so that formula credits
    /// it to the LPs (corrected by migration 007). Do not reintroduce it here.
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) protocol_fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) referral_fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) lp_fees_24h_usd: Option<Decimal>,
    /// Effective realized fee rate in basis points (`fees / volume * 10000`)
    /// over the 24h window. `None` when volume is absent or zero.
    ///
    /// NOT affected by the coverage below, and deliberately so: fees and volume
    /// are lost on exactly the same buckets (one valuation, one join), so an
    /// unvalued hour leaves the ratio's numerator AND denominator together.
    /// Only the absolute values are clipped. Do not "fix" this one.
    ///
    /// Read it precisely, though: the cancellation is exact per bucket, but this
    /// is a ratio of 24h *sums*, so it is the realized rate **of the covered
    /// hours**, not of the window. Unbiased as far as anyone knows — nothing
    /// links an hour's fee tier to whether its tokens were priced — but it is
    /// not the same statement as "the rate is unaffected".
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) effective_fee_bps: Option<Decimal>,
    /// Coverage of the five swap-derived USD figures above — `volume_24h_usd`
    /// and the four fee figures, **not** `tvl_usd`, which is valued at the
    /// latest price rather than per bucket: hours of the window that had at
    /// least one swap, and how many of them could be valued. Shipped as raw
    /// counters like `poolsPriced`/`poolsObserved` on `/api/stats` — the
    /// presentation layer turns them into a coverage label. Equal values mean
    /// full coverage; a lower numerator means the sums are sub-totals.
    #[schema(value_type = Count)]
    pub(crate) swap_buckets_24h: i64,
    #[schema(value_type = Count)]
    pub(crate) swap_buckets_priced_24h: i64,
    /// Signals emitted by this pool over the last 24h, newest first,
    /// capped per pool (service-side). Empty when the pool was quiet —
    /// the indicator's window is fixed server-side, like the 24h
    /// analytics above.
    pub(crate) signals_24h: Vec<PoolSignalResponse>,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

/// Effective realized fee rate in basis points over the window:
/// `fees / volume * 10_000`. `None` when volume is unknown or zero (no
/// meaningful rate, and avoids a division by zero).
///
/// Shared with [`super::pool_history`], which publishes the same rate per
/// bucket: it had its own inline copy of this `match` until the two were
/// merged. Unlike the fee split — which moved into SQL because it is a
/// property of the data — this one stays in the presentation layer on
/// purpose: it is a ratio of two published figures, not a fourth share, and
/// its `None` rule is about division, not about valuability.
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
    /// Compose the pool with its two enriched token sides and the
    /// derived analytics. The caller (the pool handler) is
    /// responsible for fetching the analytics for the requested
    /// pools in batch — see `enrich_pool` in `handlers/pools.rs`.
    pub(crate) fn new(
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

impl From<EnrichedToken> for EmbeddedTokenResponse {
    fn from(t: EnrichedToken) -> Self {
        EmbeddedTokenResponse::from_sources(t.mint, t.metadata, t.price)
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
