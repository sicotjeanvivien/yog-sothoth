//! Constant-product (`x·y=k`) formulas over total reserves.
//!
//! ⚠️ Dormant and wrong for concentrated liquidity (DAMM v2, DLMM): the error
//! has no fixed sign, so a result is neither a floor nor a ceiling. Replace the
//! model before wiring a caller; do not reuse the names.

/// Spot price of A in B as Q64 fixed point: `(reserve_b << 64) / reserve_a`.
/// ⚠️ Not the price of a concentrated-liquidity pool: use
/// [`super::damm_v2::sqrt_price_to_price_a_in_b`].
pub fn spot_price(reserve_a: u128, reserve_b: u128) -> Option<u128> {
    if reserve_a == 0 {
        return None;
    }

    let numerator = reserve_b.checked_shl(64)?;

    Some(numerator / reserve_a)
}

/// Price impact of a swap in basis points, under constant product.
/// ⚠️ Dormant: over-reports when liquidity sits around the price, under-reports
/// when it is parked out of range (see the module note).
pub fn price_impact(reserve_a: u128, reserve_b: u128, amount_in: u128) -> Option<u32> {
    let price_before = spot_price(reserve_a, reserve_b)?;

    let reserve_a_after = reserve_a.checked_add(amount_in)?;

    // x·y=k — reserve_b_after = k / reserve_a_after
    let k = reserve_a.checked_mul(reserve_b)?;

    let reserve_b_after = k / reserve_a_after;

    let price_after = spot_price(reserve_a_after, reserve_b_after)?;

    // price_after <= price_before for a standard swap
    let delta = price_before.saturating_sub(price_after);

    let impact_bps = delta.checked_mul(10_000)? / price_before;

    Some(impact_bps as u32)
}

/// Deviation from a 50/50 reserve split, in bps; both reserves in one unit (e.g. USD).
/// ⚠️ Dormant: a lopsided concentrated-liquidity pool is healthy, not a finding.
/// Unrelated to `FlowImbalanceDetector`, which measures USD flow.
pub fn imbalance(reserve_a: u128, reserve_b: u128) -> Option<u32> {
    let total = reserve_a.checked_add(reserve_b)?;

    if total == 0 {
        return None;
    }

    let diff = reserve_a.abs_diff(reserve_b);

    let imbalance_bps = diff.checked_mul(10_000)? / total;

    Some(imbalance_bps as u32)
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
