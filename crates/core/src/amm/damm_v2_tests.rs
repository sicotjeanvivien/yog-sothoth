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

// ── decode_base_fee_bps ─────────────────────────────────────────────────

/// Real `base_fee` bytes captured from `damm_v2_initialize_pool_2.json`:
/// a constant-fee pool, cliff_fee_numerator = 2_500_000 → 0.25 % = 25 bps,
/// mode 0 (linear scheduler, no periods).
#[test]
fn decode_base_fee_bps_constant_25bps() {
    let data: [u8; 27] = [
        160, 37, 38, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(
        decode_base_fee_bps(&data).unwrap(),
        Decimal::new(25, 0),
        "2_500_000 / 1e9 = 0.25% = 25 bps"
    );
}

/// Real `base_fee` bytes from `damm_v2_initialize_pool.json`: an anti-sniper
/// fee-scheduler pool starting at 50% — cliff_fee_numerator = 500_000_000 →
/// 5000 bps. We surface the genesis cliff, not the decayed value.
#[test]
fn decode_base_fee_bps_scheduler_cliff_5000bps() {
    let data: [u8; 27] = [
        0, 101, 205, 29, 0, 0, 0, 0, 144, 0, 88, 2, 0, 0, 0, 0, 0, 0, 196, 159, 46, 0, 0, 0, 0, 0,
        0,
    ];
    assert_eq!(decode_base_fee_bps(&data).unwrap(), Decimal::new(5000, 0));
}

/// A fractional sub-bps fee must not round: 250_000 / 1e9 = 0.000_25 = 2.5 bps.
#[test]
fn decode_base_fee_bps_fractional_is_lossless() {
    let mut data = [0u8; 27];
    data[0..8].copy_from_slice(&250_000u64.to_le_bytes());
    assert_eq!(decode_base_fee_bps(&data).unwrap(), Decimal::new(25, 1));
}

/// Rate-limiter mode (2) is accepted: the base-fee numerator is still the
/// leading u64.
#[test]
fn decode_base_fee_bps_rate_limiter_mode_ok() {
    let mut data = [0u8; 27];
    data[0..8].copy_from_slice(&1_000_000u64.to_le_bytes());
    data[BASE_FEE_MODE_OFFSET] = 2;
    assert_eq!(decode_base_fee_bps(&data).unwrap(), Decimal::new(10, 0));
}

/// An unknown mode discriminant is rejected fail-loud — never guessed.
#[test]
fn decode_base_fee_bps_unknown_mode_errors() {
    let mut data = [0u8; 27];
    data[BASE_FEE_MODE_OFFSET] = 7;
    assert!(matches!(
        decode_base_fee_bps(&data),
        Err(CoreError::FeeDecode { .. })
    ));
}

/// A truncated blob is rejected fail-loud rather than indexing past the end.
#[test]
fn decode_base_fee_bps_too_short_errors() {
    assert!(matches!(
        decode_base_fee_bps(&[0u8; 10]),
        Err(CoreError::FeeDecode { .. })
    ));
}

// ── decode_updated_base_fee_bps ─────────────────────────────────────────

/// Real `params_raw` bytes from `damm_v2_update_pool_fees.json`:
/// cliff_fee_numerator = Some(12_800_000) → 128 bps, followed by a
/// dynamic_fee (Some) and NO compounding_fee_bps field (the tx predates it)
/// — which the leading-field decode ignores.
#[test]
fn decode_updated_base_fee_bps_real_fixture_128bps() {
    let params: [u8; 42] = [
        1, 0, 80, 195, 0, 0, 0, 0, 0, 1, 1, 0, 203, 16, 199, 186, 184, 141, 6, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 10, 0, 120, 0, 136, 19, 96, 164, 220, 0, 200, 4, 0, 0,
    ];
    assert_eq!(
        decode_updated_base_fee_bps(&params).unwrap(),
        Some(Decimal::new(128, 0)),
        "12_800_000 / 1e9 = 1.28% = 128 bps"
    );
}

/// tag 0 → the update left the base fee untouched.
#[test]
fn decode_updated_base_fee_bps_none_when_unchanged() {
    // cliff None, then whatever trailing bytes — ignored.
    let params = [0u8, 1, 2, 3, 4];
    assert_eq!(decode_updated_base_fee_bps(&params).unwrap(), None);
}

/// tag 1 but fewer than 8 trailing bytes → fail-loud.
#[test]
fn decode_updated_base_fee_bps_truncated_value_errors() {
    assert!(matches!(
        decode_updated_base_fee_bps(&[1, 0, 0, 0]),
        Err(CoreError::FeeDecode { .. })
    ));
}

/// A non-0/1 Option discriminant is rejected.
#[test]
fn decode_updated_base_fee_bps_bad_tag_errors() {
    assert!(matches!(
        decode_updated_base_fee_bps(&[9, 0, 0, 0, 0, 0, 0, 0, 0]),
        Err(CoreError::FeeDecode { .. })
    ));
}

/// An empty blob is rejected.
#[test]
fn decode_updated_base_fee_bps_empty_errors() {
    assert!(matches!(
        decode_updated_base_fee_bps(&[]),
        Err(CoreError::FeeDecode { .. })
    ));
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
