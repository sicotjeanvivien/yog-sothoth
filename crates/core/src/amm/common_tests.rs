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
