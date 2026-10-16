use rust_decimal::Decimal;

use super::{base_fee_bps, max_variable_fee_bps};

/// Pool `HTvjzs…` (SOL/USDC), which Meteora displays at 0.01 %: ties the
/// formula to the product, not to the documentation.
#[test]
fn the_mainnet_sol_usdc_pool_yields_one_basis_point() {
    assert_eq!(base_fee_bps(10_000, 1, 0), Decimal::ONE);
}

/// A larger bin step is a larger price move per bin, so it carries a larger
/// fee — the two multiply.
#[test]
fn the_fee_scales_with_the_bin_step() {
    assert_eq!(base_fee_bps(10_000, 10, 0), Decimal::from(10));
    assert_eq!(base_fee_bps(10_000, 100, 0), Decimal::from(100));
}

/// Mutation: an integer return type, which rounds a sub-bp fee to 0.
#[test]
fn a_sub_basis_point_fee_keeps_its_fraction() {
    assert_eq!(base_fee_bps(5_000, 1, 0), Decimal::new(5, 1)); // 0.5 bps
    assert_eq!(base_fee_bps(1, 1, 0), Decimal::new(1, 4)); // 0.0001 bps
    assert!(base_fee_bps(1, 1, 0) > Decimal::ZERO);
}

/// The power factor is a plain factor of ten on the result.
#[test]
fn the_power_factor_multiplies_by_ten_each_step() {
    assert_eq!(base_fee_bps(100, 1, 0), Decimal::new(1, 2)); // 0.01
    assert_eq!(base_fee_bps(100, 1, 1), Decimal::new(1, 1)); // 0.1
    assert_eq!(base_fee_bps(100, 1, 2), Decimal::ONE); // 1
}

/// Mutation: removing the saturation at the chain's 10 % cap.
#[test]
fn an_absurd_power_factor_saturates_at_the_chain_cap() {
    let cap = Decimal::from(1_000);

    assert_eq!(base_fee_bps(10_000, 1, 255), cap);
    assert_eq!(base_fee_bps(u16::MAX, u16::MAX, u8::MAX), cap);
    // Saturation is not reserved for the power factor: the raw product alone
    // can exceed the cap.
    assert_eq!(base_fee_bps(u16::MAX, u16::MAX, 0), cap);
}

/// Mutation: a cap that clamps one step early.
#[test]
fn the_cap_itself_is_reached_but_not_exceeded() {
    // 10_000 × 1_000 / 10_000 = 1_000 bps = 10 %.
    assert_eq!(base_fee_bps(10_000, 1_000, 0), Decimal::from(1_000));
    assert_eq!(base_fee_bps(10_000, 999, 0), Decimal::from(999));
}

/// An all-zero account (`bin_step = 0`) decodes to a zero fee.
#[test]
fn a_zeroed_account_yields_a_zero_fee() {
    assert_eq!(base_fee_bps(0, 0, 0), Decimal::ZERO);
    assert_eq!(base_fee_bps(10_000, 0, 5), Decimal::ZERO);
}

// ── max_variable_fee_bps ────────────────────────────────────────────

/// Pool `HTvjzs…` again: a 1 bps floor that can charge three times that.
#[test]
fn the_mainnet_sol_usdc_pool_can_add_two_basis_points() {
    assert_eq!(max_variable_fee_bps(2_000_000, 100_000, 1), Decimal::TWO);
}

/// The parameters decoded from the captured DLMM account fixtures.
#[test]
fn the_captured_pools_bounds_match_their_documented_values() {
    // (variable_fee_control, max_volatility_accumulator, bin_step) → bps
    let cases = [
        ((0u32, 0u32, 1u16), Decimal::ZERO), // DZ2vZJ… — no dynamic fee
        ((2_000_000, 100_000, 1), Decimal::TWO), // HTvjzs…
        ((50_000, 150_000, 25), Decimal::new(703_125, 4)), // 8KvuP8… → 70.3125
        ((7_500, 150_000, 200), Decimal::from(675)), // 7t1sXt…
        ((10_000, 250_000, 50), Decimal::new(15_625, 2)), // JCYMX9… → 156.25
    ];

    for ((vfc, mva, bin_step), expected) in cases {
        assert_eq!(
            max_variable_fee_bps(vfc, mva, bin_step),
            expected,
            "vfc={vfc} mva={mva} bin_step={bin_step}"
        );
    }
}

/// A zero magnitude *is* "no dynamic fee": an exact zero, not a rounding artefact.
#[test]
fn a_zero_control_means_no_variable_fee() {
    assert_eq!(max_variable_fee_bps(0, 100_000, 1), Decimal::ZERO);
    assert_eq!(max_variable_fee_bps(0, u32::MAX, u16::MAX), Decimal::ZERO);
    // A zero accumulator ceiling means the same thing by the other route.
    assert_eq!(max_variable_fee_bps(2_000_000, 0, 1), Decimal::ZERO);
}

/// Mutation: `div_ceil` → plain division, which rounds this rate to zero.
#[test]
fn the_rate_rounds_up_never_down() {
    // 1 × (1 × 1)² = 1 → ceil(1 / 1e11) = 1 → 1/1e5 bps, not zero.
    assert_eq!(max_variable_fee_bps(1, 1, 1), Decimal::new(1, 5));
}

/// Inputs at their maximum widths: saturates rather than panicking.
#[test]
fn extreme_parameters_saturate_at_the_chain_cap() {
    let cap = Decimal::from(1_000);

    assert_eq!(max_variable_fee_bps(u32::MAX, u32::MAX, u16::MAX), cap);
    assert_eq!(max_variable_fee_bps(1, u32::MAX, u16::MAX), cap);
    assert_eq!(max_variable_fee_bps(u32::MAX, 100_000, 1), cap);
}

/// Mutation: a saturation that clamps one step early.
#[test]
fn the_variable_cap_is_reached_but_not_exceeded() {
    // numerator = 1e19 → ceil(1e19 / 1e11) = 1e8 → 1e8 / 1e5 = 1000 bps.
    assert_eq!(
        max_variable_fee_bps(10_000_000, 1_000_000, 1),
        Decimal::from(1_000)
    );
    assert_eq!(
        max_variable_fee_bps(9_990_000, 1_000_000, 1),
        Decimal::new(999, 0)
    );
}

/// The worst-case numerator at these input widths fits in `u128`.
#[test]
fn the_numerator_cannot_overflow_at_these_input_widths() {
    let accumulated = u128::from(u32::MAX) * u128::from(u16::MAX);
    let numerator = accumulated
        .checked_mul(accumulated)
        .and_then(|squared| squared.checked_mul(u128::from(u32::MAX)))
        .expect("the worst case must fit u128 — see the doc's margin");

    assert_eq!(
        numerator,
        340_271_982_168_772_322_334_504_870_185_799_909_375
    );
    assert!(
        numerator < u128::MAX,
        "the guards would stop being decorative"
    );
    // …and it is genuinely tight: within 0.01 % of the ceiling.
    assert!(numerator > u128::MAX / 10_000 * 9_999);
}
