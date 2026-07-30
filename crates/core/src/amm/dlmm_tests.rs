use rust_decimal::Decimal;

use super::base_fee_bps;

/// The anchor: values read from a live `LbPair` before any of this was written.
///
/// Pool `HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR` (SOL/USDC) carries
/// `base_factor = 10000`, `bin_step = 1`, `base_fee_power_factor = 0`, and
/// Meteora displays 0.01 % for it. This test is what ties the formula to the
/// product rather than to the documentation alone.
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

/// Sub-basis-point fees are representable and must not be rounded away. An
/// integer return type would have collapsed this to 0 in silence, which is why
/// `pools.fee_bps` is NUMERIC and this returns `Decimal`.
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

/// The chain caps the total fee rate at 10 %; the base fee alone cannot exceed
/// it either. Saturating keeps the function total — the caller has nowhere to
/// put an error, since `fee_bps` is not an `Option` and an unresolvable pool
/// never leaves the enrichment queue.
#[test]
fn an_absurd_power_factor_saturates_at_the_chain_cap() {
    let cap = Decimal::from(1_000);

    assert_eq!(base_fee_bps(10_000, 1, 255), cap);
    assert_eq!(base_fee_bps(u16::MAX, u16::MAX, u8::MAX), cap);
    // Saturation is not reserved for the power factor: the raw product alone
    // can exceed the cap.
    assert_eq!(base_fee_bps(u16::MAX, u16::MAX, 0), cap);
}

/// Exactly at the cap, and one step below it — the boundary is inclusive, not
/// clamped early.
#[test]
fn the_cap_itself_is_reached_but_not_exceeded() {
    // 10_000 × 1_000 / 10_000 = 1_000 bps = 10 %.
    assert_eq!(base_fee_bps(10_000, 1_000, 0), Decimal::from(1_000));
    assert_eq!(base_fee_bps(10_000, 999, 0), Decimal::from(999));
}

/// A zeroed pool account decodes to a zero fee rather than to anything
/// surprising — worth pinning, since `bin_step = 0` is what an all-zero buffer
/// looks like.
#[test]
fn a_zeroed_account_yields_a_zero_fee() {
    assert_eq!(base_fee_bps(0, 0, 0), Decimal::ZERO);
    assert_eq!(base_fee_bps(10_000, 0, 5), Decimal::ZERO);
}
