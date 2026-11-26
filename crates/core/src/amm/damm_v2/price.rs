use rust_decimal::Decimal;

use super::fee_adjusted_amount;
use crate::amm::common::price_impact;

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
    // ⚠️ A positive `f64` under `Decimal`'s smallest step (1e-28) converts to a zero `Decimal`.
    Decimal::from_f64_retain(price)
        .map(|d| d.normalize())
        .filter(|d| !d.is_zero())
}

/// Price impact of a swap after fees.
/// ⚠️ Dormant: inherits the fee errors of [`fee_adjusted_amount`] and the
/// constant-product model of [`common`](crate::amm::common). Neither a floor nor a ceiling.
pub fn net_price_impact(
    reserve_a: u128,
    reserve_b: u128,
    amount_in: u128,
    fee_bps: u32,
) -> Option<u32> {
    let amount_in_net = fee_adjusted_amount(amount_in, fee_bps)?;
    price_impact(reserve_a, reserve_b, amount_in_net)
}

#[cfg(test)]
#[path = "price_tests.rs"]
mod tests;
