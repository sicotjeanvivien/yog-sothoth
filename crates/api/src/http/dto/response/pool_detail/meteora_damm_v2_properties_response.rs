use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::amm::damm_v2::{base_fee_numerator_at, fee_numerator_to_bps};
use yog_core::domain::MeteoraDammV2PoolProperties;

use crate::http::dto::schema::UnsignedDecimal;

/// DAMM v2-only pool properties (baseline §8's satellite table).
///
/// Every field is independently optional: the fee-split percents are resolved
/// by yog-context from the on-chain account, the fee shape is decoded from the
/// genesis `InitializePool` event, and either group can be present without the
/// other.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeteoraDammV2PropertiesResponse {
    /// Fee-split percents (0..=100) from the on-chain pool account: Meteora's
    /// cut and a referrer's cut of the trading fee.
    ///
    /// There is no partner cut. `partnerFeePercent` was served here until
    /// migration 037; it decoded a padding byte and was always 0.
    #[schema(maximum = 100)]
    pub(crate) protocol_fee_percent: Option<u8>,
    #[schema(maximum = 100)]
    pub(crate) referral_fee_percent: Option<u8>,
    /// How the base fee behaves over time. Comes from the protocol
    /// satellite, written by yog-context from the on-chain account. `None`
    /// while the pool is still unresolved, or if its `BaseFeeMode` discriminant
    /// is one this crate does not map — never "no genesis event", which plays
    /// no part in filling this column.
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
    #[schema(value_type = Option<UnsignedDecimal>)]
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
    pub(super) fn build(p: MeteoraDammV2PoolProperties, evaluated_at: DateTime<Utc>) -> Self {
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
