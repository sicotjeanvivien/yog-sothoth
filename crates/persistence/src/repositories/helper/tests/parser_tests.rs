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
fn convert_optional_should_carry_absence_through() {
    let result = convert_optional(None::<i16>, "decimals", convert_i16_to_u8);

    assert_eq!(result.unwrap(), None);
}

#[test]
fn convert_optional_should_apply_the_guard_to_a_present_value() {
    let result = convert_optional(Some(200), "decimals", convert_i16_to_u8);

    assert_eq!(result.unwrap(), Some(200));
}

/// The point of the combinator: it must not swallow the converter's failure into
/// a `None`, which would turn a corrupt row into a silently missing value.
#[test]
fn convert_optional_should_propagate_the_guards_error() {
    let result = convert_optional(Some(-1), "decimals", convert_i16_to_u8);

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

/// Mutation: `to_string()`, which writes these `…e+16` and `…e+20`.
#[test]
fn convert_bigdecimal_to_u128_reads_an_integer_with_many_trailing_zeros() {
    use bigdecimal::num_bigint::BigInt;
    // sqlx reads a NUMERIC by groups of four digits: these are the shapes it builds.
    for (digits, scale, expected) in [
        (
            18_446_744_073_709_500i64,
            -16,
            184_467_440_737_095 * 10u128.pow(18),
        ),
        (1, -20, 10u128.pow(20)),
    ] {
        let value = BigDecimal::new(BigInt::from(digits), scale);

        assert_eq!(
            convert_bigdecimal_to_u128(value, "liquidity").unwrap(),
            expected,
            "{digits}e{}",
            -scale
        );
    }
}

/// Mutation: a conversion that truncates `1.5`, wraps `-1`, or narrows the range below
/// `u128::MAX`.
#[test]
fn convert_bigdecimal_to_u128_refuses_what_a_u128_cannot_hold() {
    let max = BigDecimal::from_str("340282366920938463463374607431768211455").unwrap();
    assert_eq!(
        convert_bigdecimal_to_u128(max, "liquidity").unwrap(),
        u128::MAX
    );

    for (text, reason) in [
        ("1.5", "invalid digit found in string"),
        ("-1", "invalid digit found in string"),
        (
            "340282366920938463463374607431768211456",
            "number too large to fit in target type",
        ),
    ] {
        let value = BigDecimal::from_str(text).unwrap();

        match convert_bigdecimal_to_u128(value, "liquidity") {
            Err(RepositoryError::Integrity(message)) => {
                assert_eq!(
                    message,
                    format!("liquidity parse error: {reason}"),
                    "{text}"
                )
            }
            other => panic!("{text}: {other:?}"),
        }
    }
}

#[test]
fn convert_bigdecimal_to_decimal_keeps_the_exponent_of_small_values() {
    // Mutation: `to_string()`, whose scientific form makes `Decimal::from_str`
    // drop the exponent and return 6.28.
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
    // The boundary of the case above: a short mantissa parses even through `to_string()`.
    let tiny = BigDecimal::from_str("0.000000000001").unwrap();

    let converted = convert_bigdecimal_to_decimal(tiny, "fees_usd").expect("must convert");

    assert_eq!(converted, Decimal::from_str("0.000000000001").unwrap());
}

#[test]
fn convert_bigdecimal_to_decimal_rejects_what_it_cannot_hold() {
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

/// A database error carrying only a SQLSTATE — what `map_sqlx_error` keys on.
#[derive(Debug)]
struct CodedDbError(&'static str);

impl std::fmt::Display for CodedDbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "database error {}", self.0)
    }
}

impl std::error::Error for CodedDbError {}

impl sqlx::error::DatabaseError for CodedDbError {
    fn message(&self) -> &str {
        "coded"
    }
    fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
        Some(self.0.into())
    }
    fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }
    fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
        self
    }
    fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self
    }
    fn kind(&self) -> sqlx::error::ErrorKind {
        sqlx::error::ErrorKind::Other
    }
}

#[test]
fn a_statement_cancelled_by_statement_timeout_is_a_timeout() {
    let err = SqlxError::Database(Box::new(CodedDbError("57014")));

    assert!(matches!(map_sqlx_error(err), RepositoryError::Timeout(_)));
}

/// The other half of the rule: only `57014` is a time budget. A refusal
/// (`42501`) stays a backend failure, not something a client should retry.
#[test]
fn another_database_error_stays_a_backend_failure() {
    let err = SqlxError::Database(Box::new(CodedDbError("42501")));

    assert!(matches!(map_sqlx_error(err), RepositoryError::Backend(_)));
}
