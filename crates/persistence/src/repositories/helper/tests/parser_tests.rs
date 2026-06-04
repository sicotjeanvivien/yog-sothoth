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
