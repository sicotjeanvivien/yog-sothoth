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

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── spot_price ──────────────────────────────────────────────────────────

    #[test]
    fn test_spot_price_balanced_pool() {
        // equal reserves → price = 1.0 in Q64
        let price = spot_price(1_000_000, 1_000_000).unwrap();
        let price_f64 = price as f64 / (1u128 << 64) as f64;
        assert!((price_f64 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_spot_price_sol_usdc() {
        // SOL reserve: 85 SOL (9 decimals) = 85_000_000_000
        // USDC reserve: 3_167 USDC (6 decimals) = 3_167_000_000
        // price SOL/USDC ≈ 0.0372 (inverse of ~$82/SOL — reserves in native units)
        let reserve_a = 85_000_000_000u128;
        let reserve_b = 3_167_000_000u128;
        let price = spot_price(reserve_a, reserve_b).unwrap();
        let price_f64 = price as f64 / (1u128 << 64) as f64;
        assert!((price_f64 - 0.03726).abs() < 0.001);
    }

    #[test]
    fn test_spot_price_zero_reserve_a_returns_error() {
        let result = spot_price(0, 1_000_000);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::CoreError::ArithmeticOverflow { .. })
        ));
    }

    #[test]
    fn test_spot_price_zero_reserve_b_returns_zero() {
        // reserve_b = 0 → price = 0, no error
        let price = spot_price(1_000_000, 0).unwrap();
        assert_eq!(price, 0);
    }

    // ── price_impact ────────────────────────────────────────────────────────

    #[test]
    fn test_price_impact_small_swap_is_low() {
        // 0.04 SOL into a pool with 85 SOL — small swap, low impact
        let reserve_a = 85_000_000_000u128;
        let reserve_b = 3_167_000_000u128;
        let amount_in = 40_000_000u128; // 0.04 SOL
        let impact = price_impact(reserve_a, reserve_b, amount_in).unwrap();
        // expect < 10 bps
        assert!(impact < 10, "expected impact < 10 bps, got {impact}");
    }

    #[test]
    fn test_price_impact_large_swap_is_high() {
        // swapping half the pool — very high impact
        let reserve_a = 1_000u128;
        let reserve_b = 1_000u128;
        let amount_in = 500u128; // 50% of pool
        let impact = price_impact(reserve_a, reserve_b, amount_in).unwrap();
        // expect > 1000 bps (10%)
        assert!(impact > 1000, "expected impact > 1000 bps, got {impact}");
    }

    #[test]
    fn test_price_impact_zero_amount_is_zero() {
        let reserve_a = 1_000_000u128;
        let reserve_b = 1_000_000u128;
        let impact = price_impact(reserve_a, reserve_b, 0).unwrap();
        assert_eq!(impact, 0);
    }

    // ── imbalance ───────────────────────────────────────────────────────────

    #[test]
    fn test_imbalance_balanced_pool_is_zero() {
        let result = imbalance(1_000_000, 1_000_000).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_imbalance_fully_skewed_is_max() {
        // all in one side → 10_000 bps (100%)
        let result = imbalance(1_000_000, 0).unwrap();
        assert_eq!(result, 10_000);
    }

    #[test]
    fn test_imbalance_real_pool_values() {
        // From live pool: 85 SOL vs 3167 USDC — not comparable in native units
        // but the function measures ratio deviation regardless of units
        let reserve_a = 85_000_000_000u128;
        let reserve_b = 3_167_000_000u128;
        let result = imbalance(reserve_a, reserve_b).unwrap();
        // reserve_a >> reserve_b → high imbalance
        assert!(result > 8000, "expected imbalance > 8000 bps, got {result}");
    }

    #[test]
    fn test_imbalance_zero_reserves_returns_error() {
        let result = imbalance(0, 0);
        assert!(result.is_err());
    }
}
