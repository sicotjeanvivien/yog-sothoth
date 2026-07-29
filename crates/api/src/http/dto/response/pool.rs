use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use yog_core::domain::{
    MeteoraDammV2PoolProperties, Pool, PoolAnalytics, PoolProperties, SignalRecord,
};

use crate::{
    application::{EnrichedPool, EnrichedPoolDetail, EnrichedToken},
    http::dto::EmbeddedTokenResponse,
};

/// Wire shape of one entry of a pool's recent-signals list — the
/// pools-list signal indicator. Deliberately slimmer than the feed's
/// `SignalResponse`: the indicator needs severity, kind and recency,
/// nothing else; the full signal lives on the pool's Alerts tab.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolSignalResponse {
    pub(crate) severity: String,
    pub(crate) detector: String,
    pub(crate) triggered_at: DateTime<Utc>,
}

impl From<SignalRecord> for PoolSignalResponse {
    fn from(record: SignalRecord) -> Self {
        Self {
            severity: record.signal.severity.to_string(),
            detector: record.signal.detector,
            triggered_at: record.signal.triggered_at,
        }
    }
}

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
/// window). Serialised as JSON numbers via `rust_decimal`'s exact
/// decimal representation — consistent with the price block in
/// `EmbeddedPriceResponse`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,
    pub(crate) token_a: EmbeddedTokenResponse,
    pub(crate) token_b: EmbeddedTokenResponse,
    /// Base trading fee in basis points (genesis fee tier). `None` until the
    /// pool's `InitializePool` event has been indexed.
    pub(crate) fee_bps: Option<Decimal>,
    // NOTE: the cp-amm fee properties (fee-split percents, fee shape) are NOT
    // here. They are protocol-specific, and only the pool *detail* sheet shows
    // them — see `PoolDetailResponse`. Keeping them on the list payload would
    // put one protocol's vocabulary in every protocol's row.
    pub(crate) tvl_usd: Option<Decimal>,
    pub(crate) volume_24h_usd: Option<Decimal>,
    /// Realized trading fee over the last 24h (USD), and its split: Meteora's
    /// cut, the LP cut (`fees - protocol`). `None` under the same partial-price
    /// coverage rules as `volume_24h_usd`.
    pub(crate) fees_24h_usd: Option<Decimal>,
    pub(crate) protocol_fees_24h_usd: Option<Decimal>,
    pub(crate) lp_fees_24h_usd: Option<Decimal>,
    /// Effective realized fee rate in basis points (`fees / volume * 10000`)
    /// over the 24h window. `None` when volume is absent or zero.
    pub(crate) effective_fee_bps: Option<Decimal>,
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
fn effective_fee_bps(fees_usd: Option<Decimal>, volume_usd: Option<Decimal>) -> Option<Decimal> {
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
            protocol: pool.protocol.to_string(),
            token_a,
            token_b,
            fee_bps: pool.fee_bps,
            tvl_usd: analytics.tvl_usd,
            volume_24h_usd: analytics.volume_24h_usd,
            fees_24h_usd: analytics.fees_24h_usd,
            protocol_fees_24h_usd: analytics.protocol_fees_24h_usd,
            // LP share = total realized fee minus the protocol's cut.
            lp_fees_24h_usd: match (analytics.fees_24h_usd, analytics.protocol_fees_24h_usd) {
                (Some(fees), Some(protocol)) => Some(fees - protocol),
                _ => None,
            },
            effective_fee_bps: effective_fee_bps(analytics.fees_24h_usd, analytics.volume_24h_usd),
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

/// The pool detail sheet: everything in [`PoolResponse`], plus the properties
/// that only exist for the pool's own protocol.
///
/// Flattened on the wire, so the shared fields stay at the top level exactly as
/// on the list — a client can parse a detail payload with the list schema and
/// simply ignore the extra block.
///
/// The protocol block is named after its protocol rather than tagged, so adding
/// DLMM later means adding a sibling field (`meteoraDlmm`) and no change to the
/// existing one. `None` when the pool has no satellite row yet: discovered, but
/// neither enriched by yog-context nor seen at genesis.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolDetailResponse {
    #[serde(flatten)]
    pub(crate) pool: PoolResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) meteora_damm_v2: Option<MeteoraDammV2PropertiesResponse>,
}

/// DAMM v2-only pool properties (migration 036's satellite table).
///
/// Every field is independently optional: the fee-split percents are resolved
/// by yog-context from the on-chain account, the fee shape is decoded from the
/// genesis `InitializePool` event, and either group can be present without the
/// other.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeteoraDammV2PropertiesResponse {
    /// Fee-split percents (0..=100) from the on-chain pool account: Meteora's,
    /// a partner's, and a referrer's cut of the trading fee.
    pub(crate) protocol_fee_percent: Option<u8>,
    pub(crate) partner_fee_percent: Option<u8>,
    pub(crate) referral_fee_percent: Option<u8>,
    /// How the base fee behaves over time: `constant`, `scheduler_linear`,
    /// `scheduler_exponential` or `rate_limiter`. `None` if the genesis event
    /// was never seen, or if its fee blob failed to decode.
    pub(crate) base_fee_kind: Option<String>,
    /// Whether a volatility-based dynamic fee sits on top of the base fee.
    pub(crate) has_dynamic_fee: Option<bool>,
}

impl From<MeteoraDammV2PoolProperties> for MeteoraDammV2PropertiesResponse {
    fn from(p: MeteoraDammV2PoolProperties) -> Self {
        Self {
            protocol_fee_percent: p.protocol_fee_percent,
            partner_fee_percent: p.partner_fee_percent,
            referral_fee_percent: p.referral_fee_percent,
            base_fee_kind: p.base_fee_kind,
            has_dynamic_fee: p.has_dynamic_fee,
        }
    }
}

/// The one place a pool's protocol is matched on the read path.
///
/// It belongs here and nowhere upstream: each protocol's properties have their
/// own wire shape under their own key, so the response type is *irreducibly*
/// per-protocol, while everything before it (`PoolService`, `EnrichedPoolDetail`)
/// only ever needs "this pool's properties, whatever they are".
///
/// Destructuring [`PoolProperties`] in the closure pattern is deliberate and
/// load-bearing: it is irrefutable only while the enum has one variant, so adding
/// a protocol stops compilation right here — the reminder to give it its own
/// response field. A wildcard arm would instead drop it from the wire in silence.
impl From<EnrichedPoolDetail> for PoolDetailResponse {
    fn from(d: EnrichedPoolDetail) -> Self {
        Self {
            pool: PoolResponse::from(d.pool),
            meteora_damm_v2: d
                .properties
                .map(|PoolProperties::MeteoraDammV2(p)| MeteoraDammV2PropertiesResponse::from(p)),
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_tests.rs"]
mod tests;
