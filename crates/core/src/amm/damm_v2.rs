use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::CoreResult;
use crate::amm::common::price_impact;
use crate::error::CoreError;

/// cp-amm fee denominator: a numerator `n` is the fraction `n / 1e9`.
const FEE_DENOMINATOR: u64 = 1_000_000_000;

/// How a DAMM v2 pool's **base** trading fee behaves over time.
/// ⚠️ serde's `snake_case` must match [`BaseFeeKind::as_str`], the DB and wire spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseFeeKind {
    /// Fixed fee: any scheduler mode with `number_of_period == 0`.
    Constant,
    /// Fee scheduler with linear decay (mode 0, `number_of_period > 0`).
    SchedulerLinear,
    /// Fee scheduler with exponential decay (mode 1, `number_of_period > 0`).
    SchedulerExponential,
    /// Rate limiter / anti-sniper (mode 2); its parameters are not decoded.
    RateLimiter,
    /// Market-cap scheduler with linear decay (mode 3, `number_of_period > 0`).
    MarketCapSchedulerLinear,
    /// Market-cap scheduler with exponential decay (mode 4, `number_of_period > 0`).
    MarketCapSchedulerExponential,
}

impl BaseFeeKind {
    /// Stable discriminant of the DB `base_fee_kind` column and the wire.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::SchedulerLinear => "scheduler_linear",
            Self::SchedulerExponential => "scheduler_exponential",
            Self::RateLimiter => "rate_limiter",
            Self::MarketCapSchedulerLinear => "market_cap_scheduler_linear",
            Self::MarketCapSchedulerExponential => "market_cap_scheduler_exponential",
        }
    }
}

/// Map a `BaseFeeMode` discriminant and a scheduler period count to the fee shape.
/// ⚠️ A scheduler mode with zero periods is a constant fee; mode 2 must not read
/// the period count, its layout puts `fee_increment_bps` there.
pub fn base_fee_kind_from(mode: u8, number_of_period: u16) -> CoreResult<BaseFeeKind> {
    Ok(match mode {
        // Scheduler modes with no periods never move → a constant fee.
        0 | 1 | 3 | 4 if number_of_period == 0 => BaseFeeKind::Constant,
        0 => BaseFeeKind::SchedulerLinear,
        1 => BaseFeeKind::SchedulerExponential,
        2 => BaseFeeKind::RateLimiter,
        3 => BaseFeeKind::MarketCapSchedulerLinear,
        4 => BaseFeeKind::MarketCapSchedulerExponential,
        other => {
            return Err(CoreError::FeeDecode {
                reason: format!("unknown BaseFeeMode discriminant: {other}"),
            });
        }
    })
}

/// Convert a cp-amm fee numerator to basis points, exact in `Decimal` (2_500_000 → 25).
pub fn fee_numerator_to_bps(numerator: u64) -> Decimal {
    Decimal::from(numerator) / Decimal::from(FEE_DENOMINATOR / 10_000)
}

const Q64_SCALE: f64 = 18_446_744_073_709_551_616.0; // 2^64

/// Spot price, token B per token A in human units, from the Q64.64 `sqrt_price`.
/// ⚠️ Not the reserve ratio: in concentrated liquidity reserves are not the price.
/// `f64` is fine for a display ratio; `None` unless finite and positive.
pub fn sqrt_price_to_price_a_in_b(
    sqrt_price: u128,
    decimals_a: u8,
    decimals_b: u8,
) -> Option<Decimal> {
    let ratio = sqrt_price as f64 / Q64_SCALE;
    let exponent = i32::from(decimals_a) - i32::from(decimals_b);
    let price = ratio * ratio * 10f64.powi(exponent);

    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    Decimal::from_f64_retain(price).map(|d| d.normalize())
}

/// Amount net of a fee in bps, taken off the input.
/// ⚠️ Dormant, and wrong three ways against cp-amm: `u32` truncates sub-bp tiers,
/// it rounds down (cp-amm up), and `BothToken` takes the fee off the output.
pub fn fee_adjusted_amount(amount_in: u128, fee_bps: u32) -> CoreResult<u128> {
    let fee = amount_in.checked_mul(fee_bps as u128).ok_or_else(|| {
        crate::error::CoreError::ArithmeticOverflow {
            context: "fee_adjusted_amount: amount_in * fee_bps overflows".to_string(),
        }
    })? / 10_000;

    Ok(amount_in.saturating_sub(fee))
}

/// Price impact of a swap after fees.
/// ⚠️ Dormant: inherits the fee errors of [`fee_adjusted_amount`] and the
/// constant-product model of [`super::common`]. Neither a floor nor a ceiling.
pub fn net_price_impact(
    reserve_a: u128,
    reserve_b: u128,
    amount_in: u128,
    fee_bps: u32,
) -> CoreResult<u32> {
    let amount_in_net = fee_adjusted_amount(amount_in, fee_bps)?;
    price_impact(reserve_a, reserve_b, amount_in_net)
}

// ============================================================
// Fee scheduler — the base fee as a function of time
// ============================================================

/// `cp-amm::constants::fee::MAX_BASIS_POINT`.
const MAX_BASIS_POINT: u128 = 10_000;

/// 1.0 in Q64.64 (`cp-amm::constants::ONE_Q64`).
const ONE_Q64: u128 = 1u128 << 64;

/// Q64.64 fractional bit count (`cp-amm::math::fee_math::SCALE_OFFSET`).
const SCALE_OFFSET: u32 = 64;

/// Exponent ceiling of [`pow`] (`cp-amm::math::fee_math::MAX_EXPONENTIAL`).
const MAX_EXPONENTIAL: u32 = 0x8_0000;

/// A zero period length never advances: cp-amm returns the cliff, not an error
/// (`fee_time_scheduler.rs:122`).
const ZERO_FREQUENCY_STAYS_AT_CLIFF: u64 = 0;

/// What a **time-based** fee scheduler needs to place a pool's base fee on its curve.
/// ⚠️ Only for the two time schedulers: other modes reinterpret these bytes, and
/// only the decoder refuses to build it for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeSchedulerParams {
    /// Fee numerator at period 0: the maximum of a decaying curve.
    pub cliff_fee_numerator: u64,
    /// Periods the decay runs for; past them the fee stays at the floor.
    pub number_of_period: u16,
    /// Length of one period, in the unit named by `activation_type`.
    pub period_frequency: u64,
    /// Decay per period: subtracted from the numerator (linear) or applied as
    /// `(1 - reduction_factor / 10_000)` per period (exponential).
    pub reduction_factor: u64,
    /// Start of the curve, a slot or a Unix timestamp.
    pub activation_point: u64,
    /// Unit of `activation_point` and `period_frequency`: **0 = slot, 1 = timestamp**.
    pub activation_type: u8,
    /// Linear or exponential. Any other kind must not reach this type.
    pub kind: BaseFeeKind,
}

impl FeeSchedulerParams {
    /// `activation_point + number_of_period × period_frequency`; `None` on overflow.
    pub fn expiry_point(&self) -> Option<u64> {
        u64::from(self.number_of_period)
            .checked_mul(self.period_frequency)
            .and_then(|span| self.activation_point.checked_add(span))
    }

    /// Whether the decay has finished: the fee is then at its floor, not the cliff.
    pub fn is_expired_at(&self, current_point: u64) -> bool {
        self.expiry_point()
            .is_some_and(|expiry| current_point > expiry)
    }

    /// Periods elapsed at `current_point`, clamped to `number_of_period`.
    /// ⚠️ Before activation it is `number_of_period` (the floor, not the cliff),
    /// and a zero `period_frequency` gives 0, as in cp-amm.
    fn elapsed_periods(&self, current_point: u64) -> u16 {
        if self.period_frequency == ZERO_FREQUENCY_STAYS_AT_CLIFF {
            return 0;
        }
        if current_point < self.activation_point {
            return self.number_of_period;
        }
        let elapsed = (current_point - self.activation_point) / self.period_frequency;
        // ⚠️ Saturate, never wrap: a one-second period passes `u16::MAX` within a day,
        // and wrapping to 0 would republish the cliff.
        u16::try_from(elapsed)
            .unwrap_or(u16::MAX)
            .min(self.number_of_period)
    }
}

/// The **base fee numerator in force** at `current_point`, ported from cp-amm's
/// `get_base_fee_numerator` and `get_fee_in_period`, not from the public docs.
/// `None` only on arithmetic the program would also reject.
pub fn base_fee_numerator_at(params: &FeeSchedulerParams, current_point: u64) -> Option<u64> {
    if params.period_frequency == ZERO_FREQUENCY_STAYS_AT_CLIFF {
        return Some(params.cliff_fee_numerator);
    }
    let period = params.elapsed_periods(current_point);
    match params.kind {
        BaseFeeKind::SchedulerLinear => {
            let drop = u64::from(period).checked_mul(params.reduction_factor)?;
            params.cliff_fee_numerator.checked_sub(drop)
        }
        BaseFeeKind::SchedulerExponential => {
            fee_in_period(params.cliff_fee_numerator, params.reduction_factor, period)
        }
        // ⚠️ `None`, never the cliff: the cliff is the wrong number this exists to replace.
        _ => None,
    }
}

/// `cliff × (1 - reduction_factor / 10_000) ^ passed_period`, in Q64.64;
/// verbatim port of cp-amm's `get_fee_in_period`.
fn fee_in_period(
    cliff_fee_numerator: u64,
    reduction_factor: u64,
    passed_period: u16,
) -> Option<u64> {
    if reduction_factor == 0 {
        return Some(cliff_fee_numerator);
    }
    let bps = u128::from(reduction_factor)
        .checked_shl(SCALE_OFFSET)?
        .checked_div(MAX_BASIS_POINT)?;
    let base = ONE_Q64.checked_sub(bps)?;
    let result = pow(base, i32::from(passed_period))?;
    let fee = result.checked_mul(u128::from(cliff_fee_numerator))? >> SCALE_OFFSET;
    u64::try_from(fee).ok()
}

/// Q64.64 exponentiation by squaring, ported from cp-amm's `pow`.
/// ⚠️ Not `f64::powi`: the chain's fee is this integer arithmetic, truncation
/// included. The inversion branch is unreachable here and kept for fidelity.
fn pow(base: u128, exp: i32) -> Option<u128> {
    if exp == i32::MIN {
        return None;
    }
    if exp == 0 {
        return Some(ONE_Q64);
    }

    let mut invert = exp.is_negative();
    let exp: u32 = exp.unsigned_abs();
    if exp >= MAX_EXPONENTIAL {
        return None;
    }

    let mut squared_base = base;
    let mut result = ONE_Q64;

    if squared_base >= result {
        squared_base = u128::MAX.checked_div(squared_base)?;
        invert = !invert;
    }

    // ⚠️ No squaring after the last bit: it can overflow and return `None`
    // where the chain returns a fee.
    let mut bit = 1u32;
    loop {
        if exp & bit > 0 {
            result = result.checked_mul(squared_base)? >> SCALE_OFFSET;
        }
        bit <<= 1;
        if bit >= MAX_EXPONENTIAL {
            break;
        }
        squared_base = squared_base.checked_mul(squared_base)? >> SCALE_OFFSET;
    }

    if result == 0 {
        return None;
    }
    if invert {
        result = u128::MAX.checked_div(result)?;
    }
    Some(result)
}

#[cfg(test)]
#[path = "damm_v2_tests.rs"]
mod tests;
