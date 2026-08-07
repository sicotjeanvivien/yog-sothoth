//! Unit tests for
//! `TryFrom<MeteoraDammV2PoolPropertiesRow> for MeteoraDammV2PoolProperties`.
//!
//! Pure parser tests, no DB. The percent tests here were previously in
//! `pool/rows_tests.rs` and moved with the columns in migration 036 — the guard
//! against a corrupt SMALLINT keeps its coverage.

use yog_core::{RepositoryError, domain::MeteoraDammV2PoolProperties};

use super::MeteoraDammV2PoolPropertiesRow;
use yog_core::amm::damm_v2::BaseFeeKind;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> MeteoraDammV2PoolPropertiesRow {
    MeteoraDammV2PoolPropertiesRow {
        pool_address: VALID_POOL.into(),
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
        // A constant fee has no decay curve.
        cliff_fee_numerator: None,
        number_of_period: None,
        period_frequency: None,
        reduction_factor: None,
        activation_point: None,
        activation_type: None,
    }
}

#[test]
fn try_from_valid_row_maps_every_field() {
    let props =
        MeteoraDammV2PoolProperties::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(props.pool_address.to_string(), VALID_POOL);
    assert_eq!(props.protocol_fee_percent, Some(20));
    assert_eq!(props.referral_fee_percent, Some(20));
    assert_eq!(props.base_fee_kind.as_deref(), Some("constant"));
    assert_eq!(props.has_dynamic_fee, Some(false));
}

#[test]
fn try_from_null_fee_percents_maps_to_none() {
    let row = MeteoraDammV2PoolPropertiesRow {
        protocol_fee_percent: None,
        referral_fee_percent: None,
        ..valid_row()
    };
    let props = MeteoraDammV2PoolProperties::try_from(row).expect("null percents should convert");

    assert!(props.protocol_fee_percent.is_none());
    assert!(props.referral_fee_percent.is_none());
}

/// The two column groups are written by different processes and either may land
/// first, so a row with only the fee shape — no percents — is a normal state.
#[test]
fn try_from_fee_shape_without_percents_converts() {
    let row = MeteoraDammV2PoolPropertiesRow {
        protocol_fee_percent: None,
        referral_fee_percent: None,
        base_fee_kind: Some("scheduler_linear".to_string()),
        has_dynamic_fee: Some(true),
        cliff_fee_numerator: None,
        number_of_period: None,
        period_frequency: None,
        reduction_factor: None,
        activation_point: None,
        activation_type: None,
        ..valid_row()
    };
    let props = MeteoraDammV2PoolProperties::try_from(row).expect("should convert");

    assert_eq!(props.base_fee_kind.as_deref(), Some("scheduler_linear"));
    assert_eq!(props.has_dynamic_fee, Some(true));
}

/// …and the reverse: percents resolved by yog-context before the genesis event
/// was ever seen, which is the common case (a pool's creation is only observable
/// if we were already watching).
#[test]
fn try_from_percents_without_fee_shape_converts() {
    let row = MeteoraDammV2PoolPropertiesRow {
        base_fee_kind: None,
        has_dynamic_fee: None,
        ..valid_row()
    };
    let props = MeteoraDammV2PoolProperties::try_from(row).expect("should convert");

    assert_eq!(props.protocol_fee_percent, Some(20));
    assert!(props.base_fee_kind.is_none());
}

#[test]
fn try_from_out_of_range_percent_returns_integrity() {
    let row = MeteoraDammV2PoolPropertiesRow {
        protocol_fee_percent: Some(-1),
        ..valid_row()
    };
    let err = MeteoraDammV2PoolProperties::try_from(row).expect_err("negative percent should fail");

    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = MeteoraDammV2PoolPropertiesRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = MeteoraDammV2PoolProperties::try_from(row).expect_err("bad address should fail");

    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── The decay curve, rebuilt from six columns ────────────────────────

fn scheduler_row() -> MeteoraDammV2PoolPropertiesRow {
    MeteoraDammV2PoolPropertiesRow {
        base_fee_kind: Some("scheduler_linear".to_string()),
        cliff_fee_numerator: Some(500_000_000),
        number_of_period: Some(144),
        period_frequency: Some(600),
        reduction_factor: Some(3_194_444),
        activation_point: Some(1_785_180_416),
        activation_type: Some(1),
        ..valid_row()
    }
}

#[test]
fn a_complete_scheduler_row_rebuilds_its_curve() {
    let props = MeteoraDammV2PoolProperties::try_from(scheduler_row()).unwrap();
    let s = props.fee_scheduler.expect("a complete row carries a curve");
    assert_eq!(s.cliff_fee_numerator, 500_000_000);
    assert_eq!(s.number_of_period, 144);
    assert_eq!(s.period_frequency, 600);
    assert_eq!(s.reduction_factor, 3_194_444);
    assert_eq!(s.activation_point, 1_785_180_416);
    assert_eq!(s.kind, BaseFeeKind::SchedulerLinear);
}

/// **All or nothing.** The six columns are written by one account read, so a row
/// with some set and others NULL is not a partly-usable curve — it is a corrupt
/// one. Evaluating a decay without its period length or its origin would yield a
/// confident wrong fee, which is the very defect this work removes; `None` is
/// the only honest answer, and every consumer already treats it as "no current
/// fee available".
#[test]
fn a_partial_scheduler_row_yields_no_curve_rather_than_half_of_one() {
    for (label, row) in [
        (
            "no period_frequency",
            MeteoraDammV2PoolPropertiesRow {
                period_frequency: None,
                ..scheduler_row()
            },
        ),
        (
            "no activation_point",
            MeteoraDammV2PoolPropertiesRow {
                activation_point: None,
                ..scheduler_row()
            },
        ),
        (
            "no cliff",
            MeteoraDammV2PoolPropertiesRow {
                cliff_fee_numerator: None,
                ..scheduler_row()
            },
        ),
        (
            "no reduction_factor",
            MeteoraDammV2PoolPropertiesRow {
                reduction_factor: None,
                ..scheduler_row()
            },
        ),
        (
            "no number_of_period",
            MeteoraDammV2PoolPropertiesRow {
                number_of_period: None,
                ..scheduler_row()
            },
        ),
        (
            "no activation_type",
            MeteoraDammV2PoolPropertiesRow {
                activation_type: None,
                ..scheduler_row()
            },
        ),
    ] {
        let props = MeteoraDammV2PoolProperties::try_from(row).unwrap();
        assert!(
            props.fee_scheduler.is_none(),
            "{label}: expected no curve at all"
        );
    }
}

/// The kind gate, restated on the read side because this one reads columns
/// rather than bytes: only the two time schedulers have a time curve, whatever
/// the other five columns happen to hold.
#[test]
fn a_non_time_scheduler_kind_carries_no_curve() {
    for kind in [
        "constant",
        "rate_limiter",
        "market_cap_scheduler_linear",
        "market_cap_scheduler_exponential",
    ] {
        let row = MeteoraDammV2PoolPropertiesRow {
            base_fee_kind: Some(kind.to_string()),
            ..scheduler_row()
        };
        let props = MeteoraDammV2PoolProperties::try_from(row).unwrap();
        assert!(
            props.fee_scheduler.is_none(),
            "{kind} must carry no time curve"
        );
    }
}
