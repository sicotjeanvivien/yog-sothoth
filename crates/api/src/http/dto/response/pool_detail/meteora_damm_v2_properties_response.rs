use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::amm::damm_v2::{base_fee_numerator_at, fee_numerator_to_bps};
use yog_core::domain::MeteoraDammV2PoolProperties;

use crate::http::dto::schema::UnsignedDecimal;

/// DAMM v2-only pool properties (baseline §8's satellite table).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2PropertiesResponse {
    /// Fee-split percents (0..=100) from the on-chain pool account: Meteora's
    /// cut and a referrer's cut of the trading fee.
    #[schema(maximum = 100)]
    pub(super) protocol_fee_percent: Option<u8>,
    #[schema(maximum = 100)]
    pub(super) referral_fee_percent: Option<u8>,
    /// How the base fee behaves over time, from the on-chain account. `None`
    /// while the pool is unresolved, or for a `BaseFeeMode` this crate does not map.
    pub(super) base_fee_kind: Option<String>,
    /// Whether a volatility-based dynamic fee sits on top of the base fee.
    pub(super) has_dynamic_fee: Option<bool>,

    /// The base fee in force right now, in bps; `feeBps` is the genesis tier, the
    /// top of a decaying curve. `None` without a time scheduler (constant fee,
    /// market-cap scheduler, rate limiter), for an unresolved or slot-activated pool.
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(super) current_fee_bps: Option<Decimal>,

    /// Whether the decay has finished — `currentFeeBps` will not move again.
    /// `None` under the same conditions as `currentFeeBps`.
    /// ⚠️ A zero `period_frequency` stays at its cliff while reporting finished;
    /// cp-amm's `validate` makes it unreachable on chain.
    pub(super) fee_scheduler_expired: Option<bool>,
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
