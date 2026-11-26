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

// ── base_fee_kind_from ──────────────────────────────────────────────────

/// Mutation: deciding on the mode byte alone (zero periods is a constant fee).
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

/// Mutation: mode 2 consulting the period count, which its layout reuses.
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
    assert!(base_fee_kind_from(5, 0).is_none());
    assert!(base_fee_kind_from(99, 12).is_none());
}

// ── The cap that must NOT be applied ──────────────────────────────
// ⚠️ 10 % caps an operator *update*, not a pool: v0 pools reach 50 %, v1 99 %.

/// Mutation: a clamp at 1000 bps or at the v0 cap.
#[test]
fn a_v1_ceiling_fee_converts_without_clamping() {
    assert_eq!(fee_numerator_to_bps(990_000_000), Decimal::from(9900));
}

/// Mutation: a clamp below 50 %, such as the 10 % of an operator update.
#[test]
fn a_v0_ceiling_fee_converts_without_clamping() {
    assert_eq!(fee_numerator_to_bps(500_000_000), Decimal::from(5000));
}

/// Mutation: any clamp; past every ceiling the conversion still just divides.
#[test]
fn a_numerator_past_every_ceiling_is_still_converted_not_flattened() {
    assert_eq!(fee_numerator_to_bps(1_000_000_000), Decimal::from(10_000));
}

/// Mutation: an integer conversion, which rounds 2.5 bps to 2.
/// ⚠️ Compared as `Decimal`, not as a string: the scale is a formatting detail.
#[test]
fn a_sub_bp_tier_keeps_its_fraction() {
    assert_eq!(
        fee_numerator_to_bps(250_000),
        Decimal::from_str_exact("2.5").unwrap()
    );
}
