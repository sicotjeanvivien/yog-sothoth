//! DLMM (Meteora Liquidity Book) fee arithmetic: the base fee (the floor,
//! `pools.fee_bps`) and the ceiling of the volatility-driven part on top.
//! Source: <https://docs.meteora.ag/core-products/dlmm/formulas>.

use rust_decimal::Decimal;

/// The chain's cap on `base + variable` fee at swap time (10 %), in bps.
/// Both functions here saturate at it rather than failing.
pub const MAX_FEE_BPS: Decimal = Decimal::ONE_THOUSAND;

/// Divisor of the variable-fee formula, applied to a value already in 1e9
/// precision.
const VARIABLE_FEE_SCALE: u128 = 100_000_000_000; // 1e11

/// Numerator at which the variable fee reaches [`MAX_FEE_BPS`] (`1e8 × 1e11`):
/// past it the arithmetic stops early.
const VARIABLE_FEE_SATURATION_NUMERATOR: u128 = 100_000_000 * VARIABLE_FEE_SCALE; // 1e19

/// A DLMM pool's **base** fee in bps: `base_factor × bin_step × 10^power / 10_000`.
/// The floor, as for cp-amm; unlike cp-amm, the variable part can exceed it several times.
/// ⚠️ Saturates at [`MAX_FEE_BPS`]: an `Err` would stall the resolver queue.
pub fn base_fee_bps(base_factor: u16, bin_step: u16, base_fee_power_factor: u8) -> Decimal {
    // A u32 product of two u16s — cannot overflow. Only the power factor can.
    let mut fee = Decimal::from(u32::from(base_factor) * u32::from(bin_step));
    let ceiling = MAX_FEE_BPS * Decimal::from(10_000);

    // ⚠️ Stops past the cap: `power_factor` is a `u8`, and `10^255` overflows.
    for _ in 0..base_fee_power_factor {
        if fee >= ceiling {
            return MAX_FEE_BPS;
        }
        fee *= Decimal::TEN;
    }

    (fee / Decimal::from(10_000)).min(MAX_FEE_BPS)
}

/// The **most** the volatility-driven fee can add on top of the base fee, in bps:
/// the variable-fee formula at `max_volatility_accumulator`. `0` = no dynamic fee.
/// The worst case paid is `min(base + this, MAX_FEE_BPS)`.
pub fn max_variable_fee_bps(
    variable_fee_control: u32,
    max_volatility_accumulator: u32,
    bin_step: u16,
) -> Decimal {
    // Cannot overflow at these input widths; checked against one of them growing.
    let accumulated = u128::from(max_volatility_accumulator) * u128::from(bin_step);
    let Some(squared) = accumulated.checked_mul(accumulated) else {
        return MAX_FEE_BPS;
    };
    let Some(numerator) = squared.checked_mul(u128::from(variable_fee_control)) else {
        return MAX_FEE_BPS;
    };

    // ⚠️ Also keeps the `div_ceil` below away from the u128 boundary.
    if numerator > VARIABLE_FEE_SATURATION_NUMERATOR {
        return MAX_FEE_BPS;
    }

    // The formula rounds **up** — a swapper never pays less than the rate.
    let rate_1e9 = numerator.div_ceil(VARIABLE_FEE_SCALE);

    // Guarded above, so this is at most 1e8 and fits comfortably.
    let rate = u64::try_from(rate_1e9).expect("bounded by the saturation check above");

    // Sub-bps values are real, so the fraction is kept.
    Decimal::from(rate) / Decimal::from(100_000)
}

#[cfg(test)]
#[path = "dlmm_tests.rs"]
mod tests;
