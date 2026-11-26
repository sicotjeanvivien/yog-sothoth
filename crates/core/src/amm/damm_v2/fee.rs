use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::CoreResult;
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

#[cfg(test)]
#[path = "fee_tests.rs"]
mod tests;
