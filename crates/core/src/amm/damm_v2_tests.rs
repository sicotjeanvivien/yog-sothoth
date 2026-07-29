use super::*;

// ── fee_adjusted_amount ─────────────────────────────────────────────────

#[test]
fn test_fee_adjusted_amount_25_bps() {
    // 25 bps = 0.25% fee on 1_000_000
    // fee = 1_000_000 * 25 / 10_000 = 2_500
    // net = 997_500
    let result = fee_adjusted_amount(1_000_000, 25).unwrap();
    assert_eq!(result, 997_500);
}

#[test]
fn test_fee_adjusted_amount_zero_fee() {
    // 0 bps → no fee, amount unchanged
    let result = fee_adjusted_amount(1_000_000, 0).unwrap();
    assert_eq!(result, 1_000_000);
}

#[test]
fn test_fee_adjusted_amount_max_fee() {
    // 10_000 bps = 100% fee → net = 0
    let result = fee_adjusted_amount(1_000_000, 10_000).unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_fee_adjusted_amount_real_swap() {
    // 0.04 SOL = 40_128_368 native units, fee = 25 bps
    // fee = 40_128_368 * 25 / 10_000 = 100_320
    // net = 40_028_048
    let result = fee_adjusted_amount(40_128_368, 25).unwrap();
    assert_eq!(result, 40_028_048);
}

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

// ── base_fee_kind_from ──────────────────────────────────────────────────

/// The mapping is shared by every reader of a `BaseFeeMode`, so it is tested
/// here on its own terms — the account decoder's tests cover reading the two
/// inputs out of a real layout.
///
/// A scheduler mode with zero periods is a **constant** fee: the mode byte
/// alone is never the answer.
#[test]
fn scheduler_modes_with_no_periods_are_constant() {
    for mode in [0u8, 1, 3, 4] {
        assert_eq!(
            base_fee_kind_from(mode, 0).unwrap(),
            BaseFeeKind::Constant,
            "mode {mode} with no periods"
        );
    }
}

#[test]
fn scheduler_modes_with_periods_keep_their_own_kind() {
    for (mode, expected) in [
        (0u8, BaseFeeKind::SchedulerLinear),
        (1, BaseFeeKind::SchedulerExponential),
        (3, BaseFeeKind::MarketCapSchedulerLinear),
        (4, BaseFeeKind::MarketCapSchedulerExponential),
    ] {
        assert_eq!(base_fee_kind_from(mode, 144).unwrap(), expected);
    }
}

/// Mode 2 reinterprets the bytes the schedulers use for the period count, so
/// the value passed in is meaningless and must not change the answer.
#[test]
fn the_rate_limiter_ignores_the_period_count() {
    for periods in [0u16, 144] {
        assert_eq!(
            base_fee_kind_from(2, periods).unwrap(),
            BaseFeeKind::RateLimiter
        );
    }
}

/// A mode cp-amm gains after this build is refused, never guessed.
#[test]
fn an_unknown_mode_is_refused() {
    assert!(base_fee_kind_from(5, 0).is_err());
    assert!(base_fee_kind_from(99, 12).is_err());
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
//
// Real pool states captured from the dev DB (2026-06-29), each cross-checked
// against the Jupiter oracle ratio for the pair (price_a_usd / price_b_usd).
// The decimal-adjustment exponent is the part that is easy to get wrong, so
// the assertions pin actual mainnet magnitudes, not just "it computes".

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
