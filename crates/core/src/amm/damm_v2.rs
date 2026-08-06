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
// Fee scheduler — the base fee as a function of time
// ============================================================

/// Basis-point denominator of the exponential decay factor
/// (`cp-amm::constants::fee::MAX_BASIS_POINT`).
const MAX_BASIS_POINT: u128 = 10_000;

/// 1.0 in Q64.64 (`cp-amm::constants::ONE_Q64`).
const ONE_Q64: u128 = 1u128 << 64;

/// Q64.64 fractional bit count (`cp-amm::math::fee_math::SCALE_OFFSET`).
const SCALE_OFFSET: u32 = 64;

/// Exponent ceiling of [`pow`] (`cp-amm::math::fee_math::MAX_EXPONENTIAL`).
/// Above it the Q64.64 result cannot be represented.
const MAX_EXPONENTIAL: u32 = 0x8_0000;

/// The parameters a **time-based** fee scheduler needs to place a pool's base
/// fee on its decay curve.
///
/// A named struct rather than six positional arguments: four of the six are
/// integers of similar magnitude, so a swapped pair would compile silently and
/// produce a plausible-but-wrong fee.
///
/// ⚠️ Only meaningful for [`BaseFeeKind::SchedulerLinear`] and
/// [`BaseFeeKind::SchedulerExponential`]. The market-cap schedulers decay on
/// capitalisation rather than time, and `rate_limiter` reinterprets the very
/// same account bytes — reading `period_frequency` off a mode-2 or mode-4
/// account yields garbage, which is visible on real fixtures (one returns
/// 13 722 280 043 814 587 382). The decoder is what refuses to build this
/// struct for those modes; nothing here can detect it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSchedulerParams {
    /// Fee numerator at period 0 — the **starting** fee, hence the maximum of a
    /// decaying curve.
    pub cliff_fee_numerator: u64,
    /// How many periods the decay runs for. Past it, the fee is frozen at the
    /// floor.
    pub number_of_period: u16,
    /// Length of one period, in the unit named by `activation_type`.
    pub period_frequency: u64,
    /// Decay per period: subtracted from the numerator (linear) or applied as
    /// `(1 - reduction_factor / 10_000)` per period (exponential).
    pub reduction_factor: u64,
    /// Start of the curve, a slot or a Unix timestamp.
    pub activation_point: u64,
    /// Which unit `activation_point` and `period_frequency` are in:
    /// **0 = slot, 1 = timestamp**. All eleven captured mainnet accounts use 1.
    pub activation_type: u8,
    /// Linear or exponential. Any other kind must not reach this type.
    pub kind: BaseFeeKind,
}

impl FeeSchedulerParams {
    /// The point past which the fee stops moving:
    /// `activation_point + number_of_period × period_frequency`.
    ///
    /// `None` on overflow, which no real account produces but which a garbage
    /// decode would.
    pub fn expiry_point(&self) -> Option<u64> {
        u64::from(self.number_of_period)
            .checked_mul(self.period_frequency)
            .and_then(|span| self.activation_point.checked_add(span))
    }

    /// Whether the decay has finished at `current_point` — the fee is then
    /// static at its floor, and the cliff has no relation to what a trader pays.
    ///
    /// This is the case that made the audit's measurement so wide: both pools
    /// found at ×5 and ×49 had **expired** schedulers, so the gap was permanent
    /// rather than a snapshot taken mid-decay.
    pub fn is_expired_at(&self, current_point: u64) -> bool {
        self.expiry_point()
            .is_some_and(|expiry| current_point > expiry)
    }

    /// How many periods have elapsed at `current_point`, clamped to
    /// `number_of_period`.
    ///
    /// ⚠️ **Before activation the period is `number_of_period`, not 0** — so a
    /// not-yet-activated pool sits at its **floor**, not at its cliff. That is
    /// cp-amm's behaviour (`fee_time_scheduler.rs`), it is not in the public
    /// docs, and it is the opposite of what the name "cliff" suggests.
    ///
    /// `None` when `period_frequency` is zero: cp-amm divides by it, so the
    /// on-chain program errors out there and so do we. Returning the floor
    /// instead — the first shape of this function — would have invented a fee
    /// the chain refuses to compute, in a port whose only value is fidelity.
    /// Unreachable on real data (every captured account with
    /// `period_frequency == 0` also has `number_of_period == 0`, hence a
    /// constant fee and no scheduler at all), which is exactly why it must not
    /// be papered over.
    fn elapsed_periods(&self, current_point: u64) -> Option<u16> {
        if current_point < self.activation_point {
            return Some(self.number_of_period);
        }
        let elapsed = (current_point - self.activation_point).checked_div(self.period_frequency)?;
        Some(
            u16::try_from(elapsed)
                .unwrap_or(u16::MAX)
                .min(self.number_of_period),
        )
    }
}

/// The **base fee numerator actually in force** at `current_point`.
///
/// Transcribed from cp-amm's `FeeTimeScheduler::get_base_fee_numerator`
/// (`programs/cp-amm/src/base_fee/fee_time_scheduler.rs`) and
/// `get_fee_in_period` (`math/fee_math.rs`), read from the source rather than
/// the public documentation — the docs give an approximate formula and omit
/// both the before-activation branch and the Q64.64 arithmetic.
///
/// Returns `None` only on arithmetic the on-chain program would also reject.
pub fn base_fee_numerator_at(params: &FeeSchedulerParams, current_point: u64) -> Option<u64> {
    let period = params.elapsed_periods(current_point)?;
    match params.kind {
        BaseFeeKind::SchedulerLinear => {
            let drop = u64::from(period).checked_mul(params.reduction_factor)?;
            params.cliff_fee_numerator.checked_sub(drop)
        }
        BaseFeeKind::SchedulerExponential => {
            fee_in_period(params.cliff_fee_numerator, params.reduction_factor, period)
        }
        // Not a time scheduler. Unreachable — the decoder only builds these
        // params for the two modes above — and `None` rather than the cliff on
        // purpose: the cliff is precisely the wrong number this whole ticket
        // exists to stop publishing, so a fallback that returns it would quietly
        // reinstate the defect the day this arm becomes reachable.
        _ => None,
    }
}

/// `cliff × (1 - reduction_factor / 10_000) ^ passed_period`, in Q64.64.
///
/// Verbatim port of cp-amm's `get_fee_in_period`, including its
/// `reduction_factor == 0` short-circuit.
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

/// Q64.64 exponentiation by squaring — a port of cp-amm's `pow`
/// (`math/fee_math.rs`), kept structurally faithful rather than rewritten.
///
/// ## Why not `f64::powi`
///
/// The on-chain fee is whatever this integer arithmetic yields, truncation
/// included. A floating-point approximation would disagree with the chain in
/// the last basis points — precisely the range that separates a 400 bps floor
/// from a 402 bps one, and precisely what this whole ticket is about.
///
/// ## The inversion branch
///
/// cp-amm inverts the base when `base >= 1.0` so that repeated squaring shrinks
/// instead of overflowing. **The fee scheduler never takes that branch**: its
/// base is `1 - reduction_factor/10_000`, always below 1. It is transcribed
/// anyway — dropping a branch because today's only caller cannot reach it is
/// how a port silently diverges from its source.
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

    // cp-amm unrolls this over 19 bits (0x1 … 0x40000): nineteen tests with
    // eighteen squarings *between* them. A loop is the same computation without
    // inviting a copy/paste slip on the nineteenth line — but it must not square
    // after the last test, or it can overflow and return `None` where the chain
    // returns a fee.
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

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[path = "damm_v2_tests.rs"]
mod tests;
