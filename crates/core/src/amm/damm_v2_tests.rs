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

// ── Fee scheduler ────────────────────────────────────────────────────

/// `28BDU1…`, a real mainnet `scheduler_linear` from the account fixtures:
/// cliff 5000 bps, 144 periods of 600 s (24 h), floor 400 bps.
fn fixture_28bdu1() -> FeeSchedulerParams {
    FeeSchedulerParams {
        cliff_fee_numerator: 500_000_000,
        number_of_period: 144,
        period_frequency: 600,
        reduction_factor: 3_194_444,
        activation_point: 1_785_180_416,
        activation_type: 1,
        kind: BaseFeeKind::SchedulerLinear,
    }
}

/// `FvAQ9j…`, a real `scheduler_exponential`: cliff 9900 bps (the v1 maximum),
/// 180 periods of 1 s, 326 bps of decay per period.
fn fixture_fvaq9j() -> FeeSchedulerParams {
    FeeSchedulerParams {
        cliff_fee_numerator: 990_000_000,
        number_of_period: 180,
        period_frequency: 1,
        reduction_factor: 326,
        activation_point: 1_783_799_458,
        activation_type: 1,
        kind: BaseFeeKind::SchedulerExponential,
    }
}

#[test]
fn at_activation_the_fee_is_the_cliff() {
    let p = fixture_28bdu1();
    assert_eq!(
        base_fee_numerator_at(&p, p.activation_point),
        Some(500_000_000)
    );
}

#[test]
fn linear_decay_reaches_its_floor_at_the_last_period() {
    let p = fixture_28bdu1();
    let floor = 500_000_000 - 144 * 3_194_444;
    assert_eq!(
        base_fee_numerator_at(&p, p.expiry_point().unwrap()),
        Some(floor)
    );
    assert_eq!(fee_numerator_to_bps(floor).round_dp(1).to_string(), "400.0");
}

/// The case the audit measured, and the reason this ticket exists: the pool's
/// scheduler expired on 2026-07-28, so a trader pays the 400 bps floor while
/// `pools.fee_bps` still publishes the 5000 bps cliff — a factor of 12.5.
#[test]
fn an_expired_scheduler_stays_at_its_floor_not_its_cliff() {
    let p = fixture_28bdu1();
    let long_after = p.expiry_point().unwrap() + 30 * 86_400;
    assert!(p.is_expired_at(long_after));
    let fee = base_fee_numerator_at(&p, long_after).unwrap();
    assert_eq!(fee, 500_000_000 - 144 * 3_194_444);
    assert_ne!(
        fee, p.cliff_fee_numerator,
        "the cliff must not survive expiry"
    );
}

/// cp-amm's surprise, absent from the public docs: before activation the period
/// is `number_of_period`, so the pool sits at its FLOOR, not at its cliff.
#[test]
fn before_activation_the_fee_is_the_floor_not_the_cliff() {
    let p = fixture_28bdu1();
    assert_eq!(
        base_fee_numerator_at(&p, p.activation_point - 1),
        Some(500_000_000 - 144 * 3_194_444)
    );
}

#[test]
fn exponential_decay_matches_the_chain_arithmetic() {
    let p = fixture_fvaq9j();
    // Period 0 → the cliff, untouched by the Q64.64 round trip.
    assert_eq!(
        base_fee_numerator_at(&p, p.activation_point),
        Some(990_000_000)
    );
    // Anchored on an INDEPENDENT computation, not on our own output: in float,
    // 990_000_000 * (1 - 326/10_000)^180 = 2_539_394 (25.39 bps) and the same
    // at period 90 = 50_139_808 (501.40 bps). The Q64.64 port truncates at each
    // squaring, so it lands just under; a divergence beyond a few 1e-4 would
    // mean the port, not the rounding.
    let floor = base_fee_numerator_at(&p, p.expiry_point().unwrap()).unwrap();
    assert!(
        (2_538_000..=2_540_000).contains(&floor),
        "expected ~2_539_394 (25.39 bps), got {floor} ({} bps)",
        fee_numerator_to_bps(floor)
    );
    let mid = base_fee_numerator_at(&p, p.activation_point + 90).unwrap();
    assert!(
        (50_130_000..=50_145_000).contains(&mid),
        "expected ~50_139_808 (501.40 bps), got {mid} ({} bps)",
        fee_numerator_to_bps(mid)
    );
    // Monotonic decay.
    assert!(floor < mid && mid < 990_000_000);
}

#[test]
fn a_zero_reduction_factor_never_moves() {
    let p = FeeSchedulerParams {
        reduction_factor: 0,
        ..fixture_fvaq9j()
    };
    assert_eq!(
        base_fee_numerator_at(&p, p.activation_point + 10_000),
        Some(990_000_000)
    );
}

/// A zero `period_frequency` returns the **cliff**, at any point in time.
///
/// cp-amm short-circuits on it as the first statement of
/// `get_base_fee_numerator`, before the pre-activation branch and before any
/// division: a curve whose periods have no length never advances, so it stays
/// where it started. Two earlier shapes of our port returned the floor, then
/// `None`, both from a summary of the source rather than the source itself.
///
/// Unreachable on real accounts — on-chain `validate` requires the three
/// scheduler fields to be non-zero together — which is exactly why it is pinned
/// rather than left to drift.
#[test]
fn a_zero_period_frequency_stays_at_the_cliff() {
    let p = FeeSchedulerParams {
        period_frequency: 0,
        ..fixture_28bdu1()
    };
    for point in [p.activation_point - 1, p.activation_point, u64::MAX] {
        assert_eq!(
            base_fee_numerator_at(&p, point),
            Some(500_000_000),
            "a curve that cannot advance stays at its cliff, at every point"
        );
    }
}

/// The `u16` saturation is load-bearing, and **reachable in production today**.
///
/// `FvAQ9j…` and `59cbVF…` both run a one-second period (read from their real
/// account bytes), so `u16::MAX` elapsed periods is passed about **18 hours**
/// after activation — months ago for both. A conversion that wrapped or zeroed
/// instead of saturating would send them back to period 0 and republish the
/// cliff: 9900 bps instead of 25.39, a factor of 390, which is the very defect
/// this module removes.
///
/// Mutation-checked: `unwrap_or(u16::MAX)` → `unwrap_or(0)` fails here.
#[test]
fn an_elapsed_count_past_u16_saturates_instead_of_wrapping_to_the_cliff() {
    let p = fixture_fvaq9j();
    let long_after = p.activation_point + u64::from(u16::MAX) + 1;

    let fee = base_fee_numerator_at(&p, long_after).expect("evaluable");
    assert_eq!(
        fee,
        base_fee_numerator_at(&p, p.expiry_point().unwrap()).unwrap(),
        "past the last period the fee is the floor, not the cliff"
    );
    assert_ne!(fee, p.cliff_fee_numerator);
}

/// The expiry boundary itself: `is_expired_at` is strict, so the last point of
/// the curve is not yet expired. A public field deserves its edge pinned.
#[test]
fn expiry_is_strict_at_its_own_boundary() {
    let p = fixture_28bdu1();
    let expiry = p.expiry_point().unwrap();
    assert!(
        !p.is_expired_at(expiry),
        "the last point is still on the curve"
    );
    assert!(p.is_expired_at(expiry + 1));
}

/// `pow`'s inversion branch, exercised directly.
///
/// The fee scheduler never reaches it — its base is `1 - rf/10_000`, always
/// below 1. It is transcribed anyway, and this module's own doc-comment says why:
/// "dropping a branch because today's only caller cannot reach it is how a port
/// silently diverges from its source". Leaving it untested is the same omission
/// wearing a different hat.
#[test]
fn pow_inverts_a_base_above_one_like_the_source_does() {
    // 2.0 in Q64.64, squared, is 4.0 — the branch must not change the value.
    let two = ONE_Q64 * 2;
    let four = pow(two, 2).expect("2^2 is representable");
    // Exactly 4: the inversion path computes (2^128 − 1) / (2^62 − 1), whose
    // integer ratio to ONE_Q64 is 4 on the nose. A tolerance band would accept
    // −25 % in the one module whose argument is that the last digits are the
    // chain's.
    assert_eq!(four / ONE_Q64, 4);
    // Exponent 0 is 1.0 whatever the base, on both sides of the branch.
    assert_eq!(pow(two, 0), Some(ONE_Q64));
    assert_eq!(pow(ONE_Q64 / 2, 0), Some(ONE_Q64));
}

/// The kinds that must never reach this function get `None`, never the cliff.
/// Returning the cliff would republish the exact number this ticket removes.
#[test]
fn a_non_time_scheduler_kind_yields_no_fee_not_the_cliff() {
    for kind in [
        BaseFeeKind::Constant,
        BaseFeeKind::RateLimiter,
        BaseFeeKind::MarketCapSchedulerLinear,
        BaseFeeKind::MarketCapSchedulerExponential,
    ] {
        let p = FeeSchedulerParams {
            kind,
            ..fixture_28bdu1()
        };
        assert_eq!(
            base_fee_numerator_at(&p, p.activation_point + 1_000),
            None,
            "{kind:?} must not fall back to the cliff"
        );
    }
}

// ── The cap that must NOT be applied ──────────────────────────────
//
// `core/README.md` used to state that this function "saturates at the
// chain's 10 % cap". It never did, and it must not: cp-amm's ceiling is a
// function of the pool's `fee_version` — 5000 bps (50 %) in v0, 9900 bps
// (99 %) in v1 — while 10 % is `MAX_FEE_NUMERATOR_POST_UPDATE`, the cap on
// an operator *update*, not on a pool.
//
// The danger was never the doc: it was someone reading it, finding the
// function does not match, and "fixing" the code. Clamping to 1000 bps
// would report 10 % for a legitimate anti-sniper launch pool, silently —
// `fee_bps` is an unconstrained NUMERIC and the value stays plausible.
// These assertions are what turns that into a red test.

/// The v1 ceiling, 99 %, converts whole. A clamp at any of the plausible
/// wrong values — 1000 bps, or cp-amm's own v0 cap — reddens this.
#[test]
fn a_v1_ceiling_fee_converts_without_clamping() {
    assert_eq!(fee_numerator_to_bps(990_000_000).to_string(), "9900");
}

/// And the v0 ceiling, 50 %, which the function's own doc-comment already
/// gives as an example. Kept distinct so a clamp introduced at exactly one
/// of the two caps cannot pass.
#[test]
fn a_v0_ceiling_fee_converts_without_clamping() {
    assert_eq!(fee_numerator_to_bps(500_000_000).to_string(), "5000");
}

/// Above every cp-amm ceiling the conversion still just divides. The
/// function's contract is arithmetic, not validation — an out-of-range
/// numerator is an abnormal *account*, and flattening it here would hide
/// that behind a number indistinguishable from a real 10 % tier.
#[test]
fn a_numerator_past_every_ceiling_is_still_converted_not_flattened() {
    assert_eq!(fee_numerator_to_bps(1_000_000_000).to_string(), "10000");
}

/// The other end, and the reason the return type is `Decimal`: sub-bp tiers
/// survive. An integer conversion would round 2.5 bps to 2 — the same class
/// of silent loss as the clamp, at the opposite end of the range.
///
/// Compared as a `Decimal` rather than a string: the division yields scale 2
/// (`"2.50"`), and asserting the rendering would tie this test to a formatting
/// detail instead of to the value it is about.
#[test]
fn a_sub_bp_tier_keeps_its_fraction() {
    assert_eq!(
        fee_numerator_to_bps(250_000),
        Decimal::from_str_exact("2.5").unwrap()
    );
}
