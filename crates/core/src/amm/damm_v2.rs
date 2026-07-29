use rust_decimal::Decimal;

use crate::CoreResult;
use crate::amm::common::price_impact;
use crate::error::CoreError;

/// cp-amm fee denominator: a fee numerator `n` represents the fraction
/// `n / FEE_DENOMINATOR`. 1e9 → a numerator of 2_500_000 is 0.25 %.
const FEE_DENOMINATOR: u64 = 1_000_000_000;

/// How a DAMM v2 pool's **base** trading fee behaves over time.
///
/// Decoded from the `BaseFeeMode` discriminant plus the scheduler period
/// count — the mode byte alone is not enough, since a scheduler mode with
/// zero periods is a constant fee (see [`base_fee_kind_from`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseFeeKind {
    /// Fixed fee — no scheduling (any scheduler mode with
    /// `number_of_period == 0`).
    Constant,
    /// Fee scheduler with linear decay (mode 0, `number_of_period > 0`).
    SchedulerLinear,
    /// Fee scheduler with exponential decay (mode 1, `number_of_period > 0`).
    SchedulerExponential,
    /// Rate limiter / anti-sniper (mode 2). Its internal parameters are
    /// deliberately not decoded — that layout reuses bytes 8..26 and has no
    /// captured fixture to validate against.
    RateLimiter,
    /// Market-cap scheduler with linear decay (mode 3, `number_of_period > 0`).
    MarketCapSchedulerLinear,
    /// Market-cap scheduler with exponential decay (mode 4,
    /// `number_of_period > 0`).
    MarketCapSchedulerExponential,
}

impl BaseFeeKind {
    /// Stable, lowercase discriminant for persistence / the wire. Kept in
    /// sync with the DB `base_fee_kind` column values.
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

/// Map a `BaseFeeMode` discriminant and a scheduler period count to the fee
/// *shape*.
///
/// Shared by both sources of this pair, which read the same two quantities at
/// **different offsets**: the genesis event's borsh blob (mode at 26, period
/// count at 8) and the on-chain account's zero-copy struct (mode at 16, period
/// count at 22). Only the offsets differ — the meaning does not, so the mapping
/// lives here once rather than being restated per call site.
///
/// # Why the period count is part of the decision
///
/// The mode byte alone cannot tell a constant fee from a scheduler: every
/// scheduler mode with `number_of_period == 0` never moves, and is therefore a
/// constant fee. This holds for all four scheduler modes (0, 1, 3, 4), whose
/// layouts all place `number_of_period` at the same spot within their variant.
///
/// **Mode 2 (rate limiter) is the exception** and must not consult it: its
/// layout puts `fee_increment_bps` where the schedulers keep the period count,
/// so the value passed in is meaningless for that arm.
///
/// Fails loud on an unrecognised discriminant — the caller decides whether that
/// is fatal or merely leaves the shape unknown.
pub fn base_fee_kind_from(mode: u8, number_of_period: u16) -> CoreResult<BaseFeeKind> {
    Ok(match mode {
        // Scheduler modes with no periods never move → a constant fee.
        0 | 1 | 3 | 4 if number_of_period == 0 => BaseFeeKind::Constant,
        0 => BaseFeeKind::SchedulerLinear,
        1 => BaseFeeKind::SchedulerExponential,
        // Rate limiter: `number_of_period` is not consulted — see above.
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

/// Convert a cp-amm fee numerator to basis points. The fee fraction is
/// `numerator / FEE_DENOMINATOR`; in bps that is `numerator / 100_000`. Exact
/// in `Decimal` (e.g. 2_500_000 → 25, 500_000_000 → 5000, 250_000 → 2.5).
///
/// Public because the cliff fee numerator is also read directly (as the leading
/// `u64`) from the on-chain `Pool` account by yog-context, bypassing the borsh
/// event blobs entirely.
pub fn fee_numerator_to_bps(numerator: u64) -> Decimal {
    Decimal::from(numerator) / Decimal::from(FEE_DENOMINATOR / 10_000)
}

/// Q64.64 fixed-point scale factor: the on-chain `sqrt_price` encodes
/// `sqrt(price) * 2^64`. As `f64` (exact — 2^64 is a power of two).
const Q64_SCALE: f64 = 18_446_744_073_709_551_616.0; // 2^64

/// Derive a pool's **spot price** — units of token B per 1 unit of token A, in
/// human (decimal-adjusted) terms — from the on-chain Q64.64 `sqrt_price`.
///
/// DAMM v2 is concentrated liquidity (Uniswap-v3 style), so the spot price is
/// carried by `sqrt_price`, **not** by the reserve ratio: reserves reflect
/// *where* liquidity is parked across price ranges, not the active trading
/// price. `sqrt_price` encodes `sqrt(raw_price) * 2^64`, where `raw_price` is
/// token B per token A in their raw on-chain integer units. Squaring undoes the
/// square root, dividing out the `2^64` factor undoes the fixed point, and
/// `10^(decimals_a - decimals_b)` rescales raw units to human units:
///
/// ```text
/// price_a_in_b = (sqrt_price / 2^64)^2 * 10^(decimals_a - decimals_b)
/// ```
///
/// Computed in `f64`. This is a **display / comparison ratio** (a handful of
/// significant figures), not a token quantity, so the project's lossless-integer
/// rule does not apply: `f64`'s ~15 significant digits far exceed any price's
/// display need, and squaring a `u128` exactly would overflow `Decimal` anyway.
/// Validated against real mainnet pool states (see tests). Returns `None` when
/// the result is not a finite, strictly positive number (a zero / garbage
/// `sqrt_price`, or a magnitude `Decimal` cannot hold).
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

/// Apply the DAMM v2 fee to an input amount.
///
/// Fee is expressed in basis points (1 bp = 0.01%).
/// Returns the amount net of fees.
pub fn fee_adjusted_amount(amount_in: u128, fee_bps: u32) -> CoreResult<u128> {
    let fee = amount_in.checked_mul(fee_bps as u128).ok_or_else(|| {
        crate::error::CoreError::ArithmeticOverflow {
            context: "fee_adjusted_amount: amount_in * fee_bps overflows".to_string(),
        }
    })? / 10_000;

    Ok(amount_in.saturating_sub(fee))
}

/// Compute the net price impact of a DAMM v2 swap, after fees.
///
/// DAMM v2 applies fees before the swap is executed — the effective
/// amount_in used for the x·y=k calculation is amount_in net of fees.
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
// Tests
// ============================================================

#[cfg(test)]
#[path = "damm_v2_tests.rs"]
mod tests;
