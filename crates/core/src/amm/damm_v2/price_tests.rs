use super::*;

// ── net_price_impact ────────────────────────────────────────────────────

#[test]
fn test_net_price_impact_small_swap() {
    // Same values as live swap observed in pipeline
    // 0.04 SOL into pool with 85 SOL reserve
    let reserve_a = 85_301_211_438u128; // post-swap reserve_a
    let reserve_b = 3_167_919_281u128; // post-swap reserve_b
    let amount_in = 40_128_368u128;
    let impact = net_price_impact(reserve_a, reserve_b, amount_in, 25).unwrap();
    // small swap → low impact, expect < 10 bps
    assert!(impact < 10, "expected impact < 10 bps, got {impact}");
}

#[test]
fn test_net_price_impact_higher_than_without_fee() {
    // net impact with fee should be lower than without fee
    // because fee reduces effective amount_in
    let reserve_a = 1_000_000u128;
    let reserve_b = 1_000_000u128;
    let amount_in = 100_000u128;

    let impact_with_fee = net_price_impact(reserve_a, reserve_b, amount_in, 100).unwrap();
    let impact_without_fee =
        crate::amm::common::price_impact(reserve_a, reserve_b, amount_in).unwrap();

    // with fee → less amount_in effective → lower impact
    assert!(
        impact_with_fee <= impact_without_fee,
        "impact_with_fee={impact_with_fee} should be <= impact_without_fee={impact_without_fee}"
    );
}

#[test]
fn test_net_price_impact_zero_fee_equals_price_impact() {
    // 0 bps fee → net_price_impact == price_impact
    let reserve_a = 1_000_000u128;
    let reserve_b = 1_000_000u128;
    let amount_in = 50_000u128;

    let net = net_price_impact(reserve_a, reserve_b, amount_in, 0).unwrap();
    let raw = crate::amm::common::price_impact(reserve_a, reserve_b, amount_in).unwrap();

    assert_eq!(net, raw);
}

// ── sqrt_price_to_price_a_in_b ──────────────────────────────────────────
// Real pool states, cross-checked against the oracle: they pin the decimals exponent.

/// Assert a `Decimal` price is within `rel_tol` (relative) of `expected`.
fn assert_price_approx(actual: Decimal, expected: f64, rel_tol: f64) {
    use rust_decimal::prelude::ToPrimitive;
    let a = actual.to_f64().expect("decimal fits in f64");
    assert!(
        (a - expected).abs() <= expected.abs() * rel_tol,
        "got {a}, expected ~{expected} (±{}%)",
        rel_tol * 100.0
    );
}

/// SOL (9 dec) / USDC (6 dec): oracle ≈ 71.53 USDC per SOL. Exercises a
/// non-zero decimals delta (9 − 6 = +3).
#[test]
fn sqrt_price_sol_usdc() {
    let price = sqrt_price_to_price_a_in_b(4_933_901_760_807_917_481, 9, 6).unwrap();
    assert_price_approx(price, 71.53, 0.01);
}

/// USDT (6) / USDC (6): equal decimals (exponent 0), near-parity ≈ 0.9987.
#[test]
fn sqrt_price_usdt_usdc() {
    let price = sqrt_price_to_price_a_in_b(18_435_166_270_019_141_902, 6, 6).unwrap();
    assert_price_approx(price, 0.99875, 0.001);
}

/// SOL (9) / America250 (9): a large `sqrt_price` (~1.36e21) and a high pair
/// price (~5440) — guards against overflow in the squaring path.
#[test]
fn sqrt_price_large_value_no_overflow() {
    let price = sqrt_price_to_price_a_in_b(1_360_539_537_410_322_597_216, 9, 9).unwrap();
    assert_price_approx(price, 5439.7, 0.01);
}

/// A zero `sqrt_price` has no defined price → `None`, never a fake 0.
#[test]
fn sqrt_price_zero_is_none() {
    assert!(sqrt_price_to_price_a_in_b(0, 9, 6).is_none());
}

// Mutation: no zero check after the conversion — a price under 1e-28 comes back as a fake 0.
#[test]
fn sqrt_price_below_decimal_precision_is_none() {
    assert!(sqrt_price_to_price_a_in_b(1, 6, 6).is_none());
}
