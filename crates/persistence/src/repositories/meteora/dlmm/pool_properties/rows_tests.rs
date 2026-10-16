//! DB-free tests of
//! `TryFrom<MeteoraDlmmPoolPropertiesRow> for MeteoraDlmmPoolProperties`.

use yog_core::{RepositoryError, domain::MeteoraDlmmPoolProperties};

use super::MeteoraDlmmPoolPropertiesRow;

const VALID_POOL: &str = "HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR";

/// The values read from that pool on mainnet.
fn valid_row() -> MeteoraDlmmPoolPropertiesRow {
    MeteoraDlmmPoolPropertiesRow {
        pool_address: VALID_POOL.into(),
        bin_step: Some(1),
        base_factor: Some(10_000),
        base_fee_power_factor: Some(0),
        variable_fee_control: Some(2_000_000),
        max_volatility_accumulator: Some(100_000),
        protocol_share: Some(1_000),
    }
}

#[test]
fn try_from_valid_row_maps_every_field() {
    let props = MeteoraDlmmPoolProperties::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(props.pool_address.to_string(), VALID_POOL);
    assert_eq!(props.bin_step, Some(1));
    assert_eq!(props.base_factor, Some(10_000));
    assert_eq!(props.base_fee_power_factor, Some(0));
    assert_eq!(props.variable_fee_control, Some(2_000_000));
    assert_eq!(props.max_volatility_accumulator, Some(100_000));
    assert_eq!(props.protocol_share, Some(1_000));
}

#[test]
fn try_from_an_unresolved_row_maps_every_field_to_none() {
    let row = MeteoraDlmmPoolPropertiesRow {
        pool_address: VALID_POOL.into(),
        bin_step: None,
        base_factor: None,
        base_fee_power_factor: None,
        variable_fee_control: None,
        max_volatility_accumulator: None,
        protocol_share: None,
    };
    let props = MeteoraDlmmPoolProperties::try_from(row).expect("null row should convert");

    assert!(props.bin_step.is_none());
    assert!(props.base_factor.is_none());
    assert!(props.variable_fee_control.is_none());
}

/// Mutation: a guard that rejects the top of an unsigned range.
#[test]
fn try_from_accepts_the_top_of_each_unsigned_range() {
    let row = MeteoraDlmmPoolPropertiesRow {
        bin_step: Some(65_535),
        base_factor: Some(65_535),
        base_fee_power_factor: Some(255),
        variable_fee_control: Some(4_294_967_295),
        max_volatility_accumulator: Some(4_294_967_295),
        protocol_share: Some(65_535),
        ..valid_row()
    };
    let props = MeteoraDlmmPoolProperties::try_from(row).expect("full range should convert");

    assert_eq!(props.bin_step, Some(u16::MAX));
    assert_eq!(props.base_factor, Some(u16::MAX));
    assert_eq!(props.base_fee_power_factor, Some(u8::MAX));
    assert_eq!(props.variable_fee_control, Some(u32::MAX));
    assert_eq!(props.max_volatility_accumulator, Some(u32::MAX));
    assert_eq!(props.protocol_share, Some(u16::MAX));
}

/// Mutation: a zero read as NULL — a zero base factor is a real mainnet value.
#[test]
fn try_from_distinguishes_a_zero_base_factor_from_a_missing_one() {
    let row = MeteoraDlmmPoolPropertiesRow {
        base_factor: Some(0),
        variable_fee_control: Some(0),
        ..valid_row()
    };
    let props = MeteoraDlmmPoolProperties::try_from(row).expect("should convert");

    assert_eq!(props.base_factor, Some(0));
    assert_eq!(props.variable_fee_control, Some(0));
}

#[test]
fn try_from_negative_bin_step_returns_integrity() {
    let row = MeteoraDlmmPoolPropertiesRow {
        bin_step: Some(-1),
        ..valid_row()
    };
    let err = MeteoraDlmmPoolProperties::try_from(row).expect_err("negative bin step should fail");

    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_out_of_range_variable_fee_control_returns_integrity() {
    let row = MeteoraDlmmPoolPropertiesRow {
        variable_fee_control: Some(4_294_967_296),
        ..valid_row()
    };
    let err = MeteoraDlmmPoolProperties::try_from(row).expect_err("overflow should fail");

    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = MeteoraDlmmPoolPropertiesRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = MeteoraDlmmPoolProperties::try_from(row).expect_err("bad address should fail");

    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
