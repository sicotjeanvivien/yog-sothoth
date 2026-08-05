use super::*;

#[test]
fn convert_u64_to_i64_should_convert_valid_value() {
    let result = convert_u64_to_i64(42, "amount");

    assert_eq!(result.unwrap(), 42);
}

#[test]
fn convert_u64_to_i64_should_fail_on_overflow() {
    let result = convert_u64_to_i64(u64::MAX, "amount");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i64_to_u64_should_convert_valid_value() {
    let result = convert_i64_to_u64(42, "amount");

    assert_eq!(result.unwrap(), 42);
}

#[test]
fn convert_i64_to_u64_should_fail_for_negative_value() {
    let result = convert_i64_to_u64(-1, "amount");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i16_to_u8_should_convert_valid_value() {
    let result = convert_i16_to_u8(9, "decimals");

    assert_eq!(result.unwrap(), 9);
}

#[test]
fn convert_i16_to_u8_should_fail_for_negative_value() {
    let result = convert_i16_to_u8(-1, "decimals");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i16_to_u8_should_fail_on_overflow() {
    let result = convert_i16_to_u8(256, "decimals");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i32_to_u16_should_convert_valid_value() {
    let result = convert_i32_to_u16(400, "bin_step");

    assert_eq!(result.unwrap(), 400);
}

/// The whole point of the INTEGER width: a `u16` that SMALLINT could not hold.
#[test]
fn convert_i32_to_u16_should_accept_the_top_of_the_range() {
    let result = convert_i32_to_u16(65_535, "bin_step");

    assert_eq!(result.unwrap(), u16::MAX);
}

#[test]
fn convert_i32_to_u16_should_fail_for_negative_value() {
    let result = convert_i32_to_u16(-1, "bin_step");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i32_to_u16_should_fail_on_overflow() {
    let result = convert_i32_to_u16(65_536, "bin_step");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i64_to_u32_should_convert_valid_value() {
    let result = convert_i64_to_u32(2_000_000, "variable_fee_control");

    assert_eq!(result.unwrap(), 2_000_000);
}

/// Likewise for BIGINT: a `u32` past `i32::MAX` must survive the round trip.
#[test]
fn convert_i64_to_u32_should_accept_the_top_of_the_range() {
    let result = convert_i64_to_u32(4_294_967_295, "variable_fee_control");

    assert_eq!(result.unwrap(), u32::MAX);
}

#[test]
fn convert_i64_to_u32_should_fail_for_negative_value() {
    let result = convert_i64_to_u32(-1, "variable_fee_control");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_i64_to_u32_should_fail_on_overflow() {
    let result = convert_i64_to_u32(4_294_967_296, "variable_fee_control");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_optional_should_carry_absence_through() {
    let result = convert_optional(None::<i32>, "bin_step", convert_i32_to_u16);

    assert_eq!(result.unwrap(), None);
}

#[test]
fn convert_optional_should_apply_the_guard_to_a_present_value() {
    let result = convert_optional(Some(400), "bin_step", convert_i32_to_u16);

    assert_eq!(result.unwrap(), Some(400));
}

/// The point of the combinator: it must not swallow the converter's failure into
/// a `None`, which would turn a corrupt row into a silently missing value.
#[test]
fn convert_optional_should_propagate_the_guards_error() {
    let result = convert_optional(Some(-1), "bin_step", convert_i32_to_u16);

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_u128_to_bigdecimal_should_convert() {
    let value = 12345678901234567890u128;

    let result = convert_u128_to_bigdecimal(value, "price");

    assert_eq!(result.to_string(), value.to_string());
}

#[test]
fn convert_bigdecimal_to_u128_should_convert() {
    let value = BigDecimal::from_str("123456789").unwrap();

    let result = convert_bigdecimal_to_u128(value, "price").unwrap();

    assert_eq!(result, 123456789u128);
}

#[test]
fn convert_bigdecimal_to_u128_should_fail_for_invalid_value() {
    let value = BigDecimal::from_str("-1").unwrap();

    let result = convert_bigdecimal_to_u128(value, "price");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_bigdecimal_to_decimal_keeps_the_exponent_of_small_values() {
    // Found in review of PR #106. `BigDecimal::to_string()` renders this as
    // `6.283855409573290776726186454503737E-12`, and `Decimal::from_str` on
    // that form fills its 28-digit scale with the mantissa, DROPS the exponent
    // and returns `Ok` — 6.28e-12 became 6.28, a factor of 10^12, silently.
    //
    // A 6.28e-12 USD fee published as 6.28 USD is exactly the defect the
    // coverage counters exist to prevent, one layer up: a number that is not
    // what it claims to be, with nothing saying so.
    let tiny = BigDecimal::from_str("0.000000000006283855409573290776726186454503737").unwrap();

    let converted = convert_bigdecimal_to_decimal(tiny, "fees_usd").expect("must convert");

    assert!(
        converted < Decimal::from_str("0.000000001").unwrap(),
        "a picodollar must stay a picodollar, got {converted}"
    );
    assert_eq!(
        converted,
        Decimal::from_str("0.0000000000062838554095732908").unwrap(),
        "value truncated to Decimal's 28-digit scale, exponent intact"
    );
}

#[test]
fn convert_bigdecimal_to_decimal_survives_a_short_mantissa_in_scientific_form() {
    // The case that always worked, kept as the boundary of the one above: a
    // short mantissa leaves room for the exponent, so `to_string()` parsed
    // fine. That is why the defect stayed dormant for so long.
    let tiny = BigDecimal::from_str("0.000000000001").unwrap();

    let converted = convert_bigdecimal_to_decimal(tiny, "fees_usd").expect("must convert");

    assert_eq!(converted, Decimal::from_str("0.000000000001").unwrap());
}

#[test]
fn convert_bigdecimal_to_decimal_rejects_what_it_cannot_hold() {
    // The other half of the fix: `to_plain_string` never hides an exponent, so
    // an out-of-range value now fails loudly instead of being truncated into a
    // plausible wrong number.
    let huge = BigDecimal::from_str("1000000000000000000000000000000").unwrap();

    let err = convert_bigdecimal_to_decimal(huge, "tvl_usd")
        .expect_err("beyond Decimal's range must be an error, never a guess");

    match err {
        RepositoryError::Integrity(m) => assert!(m.contains("tvl_usd"), "got: {m}"),
        other => panic!("expected Integrity, got {other:?}"),
    }
}

#[test]
fn convert_bigdecimal_to_decimal_should_convert() {
    let value = BigDecimal::from_str("123.456").unwrap();

    let result = convert_bigdecimal_to_decimal(value, "amount").unwrap();

    assert_eq!(result, Decimal::from_str("123.456").unwrap());
}

#[test]
fn parse_string_to_liquidity_event_kind_should_convert() {
    let result = parse_string_to_liquidity_event_kind("add".to_string(), "kind");

    assert!(result.is_ok());
}

#[test]
fn parse_string_to_liquidity_event_kind_should_fail() {
    let result = parse_string_to_liquidity_event_kind("foobar".to_string(), "kind");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_string_to_pubkey_should_convert() {
    let key = Pubkey::new_unique();

    let result = convert_string_to_pubkey(key.to_string(), "pool_address").unwrap();

    assert_eq!(result, key);
}

#[test]
fn convert_string_to_pubkey_should_fail() {
    let result = convert_string_to_pubkey("invalid".to_string(), "pool_address");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_string_to_signature_should_fail() {
    let result = convert_string_to_signature("invalid".to_string(), "signature");

    assert!(matches!(result, Err(RepositoryError::Integrity(_))));
}

#[test]
fn convert_string_to_signature_should_convert() {
    let signature = Signature::from([2; 64]);

    let result = convert_string_to_signature(signature.to_string(), "signature").unwrap();

    assert_eq!(result, signature);
}
