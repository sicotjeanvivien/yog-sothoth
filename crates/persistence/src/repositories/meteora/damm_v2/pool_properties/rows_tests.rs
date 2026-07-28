//! Unit tests for
//! `TryFrom<MeteoraDammV2PoolPropertiesRow> for MeteoraDammV2PoolProperties`.
//!
//! Pure parser tests, no DB. The percent tests here were previously in
//! `pool/rows_tests.rs` and moved with the columns in migration 036 — the guard
//! against a corrupt SMALLINT keeps its coverage.

use yog_core::{RepositoryError, domain::MeteoraDammV2PoolProperties};

use super::MeteoraDammV2PoolPropertiesRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> MeteoraDammV2PoolPropertiesRow {
    MeteoraDammV2PoolPropertiesRow {
        pool_address: VALID_POOL.into(),
        protocol_fee_percent: Some(20),
        partner_fee_percent: Some(0),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
    }
}

#[test]
fn try_from_valid_row_maps_every_field() {
    let props =
        MeteoraDammV2PoolProperties::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(props.pool_address.to_string(), VALID_POOL);
    assert_eq!(props.protocol_fee_percent, Some(20));
    assert_eq!(props.partner_fee_percent, Some(0));
    assert_eq!(props.referral_fee_percent, Some(20));
    assert_eq!(props.base_fee_kind.as_deref(), Some("constant"));
    assert_eq!(props.has_dynamic_fee, Some(false));
}

#[test]
fn try_from_null_fee_percents_maps_to_none() {
    let row = MeteoraDammV2PoolPropertiesRow {
        protocol_fee_percent: None,
        partner_fee_percent: None,
        referral_fee_percent: None,
        ..valid_row()
    };
    let props = MeteoraDammV2PoolProperties::try_from(row).expect("null percents should convert");

    assert!(props.protocol_fee_percent.is_none());
    assert!(props.partner_fee_percent.is_none());
    assert!(props.referral_fee_percent.is_none());
}

/// The two column groups are written by different processes and either may land
/// first, so a row with only the fee shape — no percents — is a normal state.
#[test]
fn try_from_fee_shape_without_percents_converts() {
    let row = MeteoraDammV2PoolPropertiesRow {
        protocol_fee_percent: None,
        partner_fee_percent: None,
        referral_fee_percent: None,
        base_fee_kind: Some("scheduler_linear".to_string()),
        has_dynamic_fee: Some(true),
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
