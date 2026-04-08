use crate::{CoreError, CoreResult};

/// Compute the spot price of token A in terms of token B as a Q64 fixed-point integer.
///
/// Formula: price_q64 = (reserve_b << 64) / reserve_a
///
/// Convert to f64 for display only:
///   price = price_q64 as f64 / (1u128 << 64) as f64
pub fn spot_price(reserve_a: u128, reserve_b: u128) -> CoreResult<u128> {
    if reserve_a == 0 {
        return Err(CoreError::ArithmeticOverflow {
            context: "spot_price: reserve_a is zero".to_string(),
        });
    }

    let numerator = reserve_b
        .checked_shl(64)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "spot_price: reserve_b << 64 overflows u128".to_string(),
        })?;

    Ok(numerator / reserve_a)
}

/// Compute the price impact of a swap in basis points (1 bp = 0.01%).
///
/// Formula: impact_bps = ((price_after - price_before) / price_before) * 10_000
///
/// Uses Q64 prices to stay in integer arithmetic throughout.
pub fn price_impact(reserve_a: u128, reserve_b: u128, amount_in: u128) -> CoreResult<u32> {
    let price_before = spot_price(reserve_a, reserve_b)?;

    let reserve_a_after =
        reserve_a
            .checked_add(amount_in)
            .ok_or_else(|| CoreError::ArithmeticOverflow {
                context: "price_impact: reserve_a + amount_in overflows".to_string(),
            })?;

    // x·y=k — reserve_b_after = k / reserve_a_after
    let k = reserve_a
        .checked_mul(reserve_b)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "price_impact: reserve_a * reserve_b overflows".to_string(),
        })?;

    let reserve_b_after = k / reserve_a_after;

    let price_after = spot_price(reserve_a_after, reserve_b_after)?;

    // price_after <= price_before for a standard swap
    let delta = price_before.saturating_sub(price_after);

    let impact_bps = delta
        .checked_mul(10_000)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "price_impact: delta * 10_000 overflows".to_string(),
        })?
        / price_before;

    Ok(impact_bps as u32)
}

/// Compute the pool imbalance in basis points (1 bp = 0.01%).
///
/// Measures how far the pool deviates from a 50/50 reserve ratio.
/// A perfectly balanced pool returns 0 bps.
///
/// Both reserves must be expressed in the same unit (e.g. USD value)
/// for this metric to be meaningful.
pub fn imbalance(reserve_a: u128, reserve_b: u128) -> CoreResult<u32> {
    let total = reserve_a
        .checked_add(reserve_b)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "imbalance: reserve_a + reserve_b overflows".to_string(),
        })?;

    if total == 0 {
        return Err(CoreError::ArithmeticOverflow {
            context: "imbalance: total reserves are zero".to_string(),
        });
    }

    let diff = reserve_a.abs_diff(reserve_b);

    let imbalance_bps = diff
        .checked_mul(10_000)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "imbalance: diff * 10_000 overflows".to_string(),
        })?
        / total;

    Ok(imbalance_bps as u32)
}
