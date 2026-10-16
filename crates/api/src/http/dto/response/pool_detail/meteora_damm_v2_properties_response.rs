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
    /// Builds the response, evaluating the fee curve at `evaluated_at` — not a `From`, which
    /// would read the clock. A slot-activated curve is `None`: no current slot is read here.
    pub(super) fn build(p: MeteoraDammV2PoolProperties, evaluated_at: DateTime<Utc>) -> Self {
        // ⚠️ Both fields from one evaluation: derived apart, a curve the chain refuses would read
        // as expired with no fee.
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
