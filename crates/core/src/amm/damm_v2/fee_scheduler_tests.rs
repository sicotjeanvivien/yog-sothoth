use super::*;
use crate::amm::damm_v2::fee_numerator_to_bps;

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

/// An expired scheduler: the trader pays the 400 bps floor, not the 5000 bps cliff.
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

/// Mutation: a pre-activation period of 0, which publishes the cliff, not the floor.
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
    // Anchored on a float computation (2_539_394 at period 180, 50_139_808 at 90):
    // the port truncates, so it lands just under, within a few 1e-4.
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
/// Mutation: returning the floor or `None` instead of cp-amm's short-circuit.
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

/// A real one-second period, long past `u16::MAX` elapsed periods.
/// Mutation: `unwrap_or(u16::MAX)` → `unwrap_or(0)`, which republishes the cliff.
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

/// Mutation: `>` → `>=` in `is_expired_at`; the last point is not yet expired.
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

/// `pow`'s inversion branch, which the fee scheduler never reaches.
#[test]
fn pow_inverts_a_base_above_one_like_the_source_does() {
    // 2.0 in Q64.64, squared, is 4.0 — the branch must not change the value.
    let two = ONE_Q64 * 2;
    let four = pow(two, 2).expect("2^2 is representable");
    // Exactly 4, no tolerance band: the last digits are the chain's.
    assert_eq!(four / ONE_Q64, 4);
    // Exponent 0 is 1.0 whatever the base, on both sides of the branch.
    assert_eq!(pow(two, 0), Some(ONE_Q64));
    assert_eq!(pow(ONE_Q64 / 2, 0), Some(ONE_Q64));
}

/// Mutation: the fallback arm returning the cliff instead of `None`.
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
