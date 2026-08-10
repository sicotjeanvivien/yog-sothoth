use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use yog_core::amm::damm_v2::{base_fee_numerator_at, fee_numerator_to_bps};
use yog_core::domain::{
    MeteoraDammV2PoolProperties, MeteoraDlmmPoolProperties, Pool, PoolAnalytics, PoolProperties,
    SignalRecord,
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
/// window). Serialised as JSON **strings** via `rust_decimal`'s default
/// representation — consistent with the price block in `EmbeddedPriceResponse`,
/// and with the web's `BigDecimal` zod type, which keeps the trailing digits a
/// JS `number` would drop. (This said "JSON numbers" until a serialisation test
/// pinned the real shape.)
///
/// A non-null 24h figure is not necessarily a *complete* one: the sum skips
/// the hours it cannot value. `swapBuckets24h` / `swapBucketsPriced24h` carry
/// that coverage so a partial total cannot pass for a full one.
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
    /// Realized trading fee over the last 24h (USD), and its three shares:
    /// Meteora's cut, the referrer's cut, and what is left for the LPs. They
    /// sum back to `fees_24h_usd` exactly, and are `None` together with it,
    /// under the same partial-price coverage rules as `volume_24h_usd`.
    ///
    /// All four are read straight from the analytics — the split is computed
    /// once in SQL. `lp_fees_24h_usd` is **not** `fees - protocol`: cp-amm
    /// takes the referral out of the protocol share, so that formula credits
    /// it to the LPs (`.project` ticket 05). Do not reintroduce it here.
    pub(crate) fees_24h_usd: Option<Decimal>,
    pub(crate) protocol_fees_24h_usd: Option<Decimal>,
    pub(crate) referral_fees_24h_usd: Option<Decimal>,
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
    pub(crate) effective_fee_bps: Option<Decimal>,
    /// Coverage of the five swap-derived USD figures above — `volume_24h_usd`
    /// and the four fee figures, **not** `tvl_usd`, which is valued at the
    /// latest price rather than per bucket: hours of the window that had at
    /// least one swap, and how many of them could be valued. Shipped as raw
    /// counters like `poolsPriced`/`poolsObserved` on `/api/stats` — the
    /// presentation layer turns them into a coverage label. Equal values mean
    /// full coverage; a lower numerator means the sums are sub-totals.
    pub(crate) swap_buckets_24h: i64,
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
            protocol: pool.protocol.to_string(),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) meteora_dlmm: Option<MeteoraDlmmPropertiesResponse>,
}

/// DAMM v2-only pool properties (baseline §8's satellite table).
///
/// Every field is independently optional: the fee-split percents are resolved
/// by yog-context from the on-chain account, the fee shape is decoded from the
/// genesis `InitializePool` event, and either group can be present without the
/// other.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeteoraDammV2PropertiesResponse {
    /// Fee-split percents (0..=100) from the on-chain pool account: Meteora's
    /// cut and a referrer's cut of the trading fee.
    ///
    /// There is no partner cut. `partnerFeePercent` was served here until
    /// migration 037; it decoded a padding byte and was always 0.
    pub(crate) protocol_fee_percent: Option<u8>,
    pub(crate) referral_fee_percent: Option<u8>,
    /// How the base fee behaves over time: `constant`, `scheduler_linear`,
    /// `scheduler_exponential` or `rate_limiter`. `None` if the genesis event
    /// was never seen, or if its fee blob failed to decode.
    pub(crate) base_fee_kind: Option<String>,
    /// Whether a volatility-based dynamic fee sits on top of the base fee.
    pub(crate) has_dynamic_fee: Option<bool>,

    /// The base fee **actually in force right now**, in bps, for a pool whose
    /// fee decays over time.
    ///
    /// `feeBps` above is the genesis tier — the fee at period 0, which for a
    /// scheduler is the *maximum* of a decreasing curve. This is the same curve
    /// evaluated at read time, and the two differ by up to ×49 on real pools.
    ///
    /// `None` whenever it cannot be established honestly: no scheduler (a
    /// constant fee already tells the whole truth), a market-cap scheduler or a
    /// rate limiter (neither decays on time), an unresolved account — or a
    /// **slot-activated** pool, see below.
    pub(crate) current_fee_bps: Option<Decimal>,

    /// Whether the decay has finished — `currentFeeBps` will not move again.
    /// `None` under the same conditions as above.
    ///
    /// It is the *floor* in every case a real account can produce. The one
    /// exception is a curve with a zero `period_frequency`, which never advances
    /// and stays at its **cliff** while still reporting as finished — cp-amm
    /// says the same, and its `validate` makes the combination unreachable on
    /// chain.
    pub(crate) fee_scheduler_expired: Option<bool>,
}

impl MeteoraDammV2PropertiesResponse {
    /// Build the response, evaluating the fee curve at `evaluated_at`.
    ///
    /// Not a `From` impl because the current fee is a function of *when* it is
    /// asked for; a conversion that reads the clock itself could not be tested.
    ///
    /// ## Slot-activated pools return `None`, deliberately
    ///
    /// `activation_type` names the unit of the curve: 0 = slot, 1 = timestamp.
    /// A timestamp curve is evaluated against the clock, which this has. A slot
    /// curve would need the current Solana slot — `network_status` holds one,
    /// but wiring that lookup into this service is a dependency this change does
    /// not need: **all eleven captured mainnet accounts are timestamp-activated**,
    /// so the slot branch has never been observed. Returning `None` says "not
    /// established" rather than inventing a fee, and the seam is one field wide
    /// if a slot-activated pool ever shows up.
    fn build(p: MeteoraDammV2PoolProperties, evaluated_at: DateTime<Utc>) -> Self {
        // Both fields come from ONE successful evaluation, deliberately.
        //
        // Deriving `expired` independently — `point.map(...)` next to the
        // `and_then` below — let the pair disagree: an arithmetic the chain also
        // refuses (a linear curve whose total decay exceeds its cliff) yields no
        // fee while still reporting `expired: true`, which contradicts what this
        // field documents. A consumer reading "the decay is over" alongside a
        // null fee has been told two incompatible things about the same pool.
        let evaluated = p
            .fee_scheduler
            .filter(|s| s.activation_type == TIMESTAMP_ACTIVATION)
            .map(|s| (s, evaluated_at.timestamp().max(0) as u64))
            .and_then(|(s, now)| {
                base_fee_numerator_at(&s, now).map(|fee| (fee, s.is_expired_at(now)))
            });

        let current_fee_bps = evaluated.map(|(fee, _)| fee_numerator_to_bps(fee));
        let fee_scheduler_expired = evaluated.map(|(_, expired)| expired);

        Self {
            protocol_fee_percent: p.protocol_fee_percent,
            referral_fee_percent: p.referral_fee_percent,
            base_fee_kind: p.base_fee_kind,
            has_dynamic_fee: p.has_dynamic_fee,
            current_fee_bps,
            fee_scheduler_expired,
        }
    }
}

/// `activation_type` value meaning "the curve is measured in Unix seconds".
const TIMESTAMP_ACTIVATION: u8 = 1;

/// DLMM-only pool properties (baseline §9's satellite table).
///
/// Every field is optional together, not independently: all six come from one
/// read of one `LbPair` account, so they are present for a resolved pool and
/// absent for one yog-context has not reached.
///
/// The pool's *state* — the active bin, the volatility accumulator — is not
/// here. It moves on every swap and belongs to the current-state surface.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeteoraDlmmPropertiesResponse {
    /// Price increment between adjacent bins, in basis points: bin `i` sits at
    /// `(1 + binStep / 10_000)^i`. The defining property of a DLMM pool.
    pub(crate) bin_step: Option<u16>,
    /// The two other inputs to the pool's base fee, served raw so a client can
    /// recompute `feeBps` rather than trust it: `baseFactor × binStep ×
    /// 10^baseFeePowerFactor / 10_000`.
    pub(crate) base_factor: Option<u16>,
    pub(crate) base_fee_power_factor: Option<u8>,
    /// Magnitude of the volatility-driven fee charged on top of the base fee.
    /// **Zero means no dynamic fee** — DLMM has no boolean flag, unlike DAMM
    /// v2's `hasDynamicFee`.
    pub(crate) variable_fee_control: Option<u32>,
    /// Per-pool ceiling on the volatility accumulator, and so on how far the
    /// variable fee can climb.
    pub(crate) max_volatility_accumulator: Option<u32>,
    /// Meteora's cut of the trading fee, in **basis points** — not the whole
    /// percent DAMM v2's `protocolFeePercent` uses. The two are not comparable
    /// without scaling.
    pub(crate) protocol_share: Option<u16>,
}

impl From<MeteoraDlmmPoolProperties> for MeteoraDlmmPropertiesResponse {
    fn from(p: MeteoraDlmmPoolProperties) -> Self {
        Self {
            bin_step: p.bin_step,
            base_factor: p.base_factor,
            base_fee_power_factor: p.base_fee_power_factor,
            variable_fee_control: p.variable_fee_control,
            max_volatility_accumulator: p.max_volatility_accumulator,
            protocol_share: p.protocol_share,
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
/// The exhaustive `match` is deliberate and load-bearing: adding a protocol
/// stops compilation right here — the reminder to give it its own response
/// field. A wildcard arm would instead drop it from the wire in silence. This is
/// what happened when DLMM landed: the previously irrefutable
/// `|PoolProperties::MeteoraDammV2(p)|` closure stopped compiling, which is
/// exactly the intended behaviour.
impl From<EnrichedPoolDetail> for PoolDetailResponse {
    fn from(d: EnrichedPoolDetail) -> Self {
        let (meteora_damm_v2, meteora_dlmm) = match d.properties {
            Some(PoolProperties::MeteoraDammV2(p)) => (
                Some(MeteoraDammV2PropertiesResponse::build(p, d.evaluated_at)),
                None,
            ),
            Some(PoolProperties::MeteoraDlmm(p)) => {
                (None, Some(MeteoraDlmmPropertiesResponse::from(p)))
            }
            None => (None, None),
        };

        Self {
            pool: PoolResponse::from(d.pool),
            meteora_damm_v2,
            meteora_dlmm,
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_tests.rs"]
mod tests;
