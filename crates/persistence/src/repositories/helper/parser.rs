use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::Error as SqlxError;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{RepositoryError, RepositoryResult, domain::Protocol};

/// Parses a stored protocol; an unknown one is `RepositoryError::Integrity`.
pub(crate) fn parse_protocol(raw: &str) -> RepositoryResult<Protocol> {
    Protocol::from_str(raw)
        .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))
}

/// The bind of an optional protocol filter: `None` reads every protocol.
pub(crate) fn bind_protocol_filter(protocol: Option<Protocol>) -> Option<&'static str> {
    protocol.map(|p| p.as_str())
}

/// The start of a bucket a read selected; `None` is `RepositoryError::Integrity`.
pub(crate) fn parse_bucket_from(from: Option<DateTime<Utc>>) -> RepositoryResult<DateTime<Utc>> {
    from.ok_or_else(|| RepositoryError::Integrity("a bucket read without its start".to_owned()))
}

/// Parses a stored pubkey; a malformed one is `RepositoryError::Integrity`.
pub(crate) fn convert_string_to_pubkey(key: String, field: &str) -> RepositoryResult<Pubkey> {
    Pubkey::from_str(&key)
        .map_err(|e| RepositoryError::Integrity(format!("invalid {field} pubkey: {e}")))
}

/// Parses a stored transaction signature; a malformed one is `RepositoryError::Integrity`.
pub(crate) fn convert_string_to_signature(raw: String, field: &str) -> RepositoryResult<Signature> {
    Signature::from_str(&raw)
        .map_err(|e| RepositoryError::Integrity(format!("invalid {field} signature: {e}")))
}

/// Narrows an `INTEGER` into a `u16`; fails outside `0..=65535`.
pub(crate) fn convert_i32_to_u16(v: i32, field: &str) -> RepositoryResult<u16> {
    u16::try_from(v).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {v}")))
}

/// Narrows a `BIGINT` into a `u32`; fails outside `0..=u32::MAX`.
pub(crate) fn convert_i64_to_u32(v: i64, field: &str) -> RepositoryResult<u32> {
    u32::try_from(v).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {v}")))
}

/// Converts a `u64` for a `BIGINT` column; fails past `i64::MAX`.
pub(crate) fn convert_u64_to_i64(v: u64, field: &str) -> RepositoryResult<i64> {
    i64::try_from(v).map_err(|e| RepositoryError::Integrity(format!("invalid {field}: {e}")))
}

/// Converts a `BIGINT` into a `u64`; fails on a negative value.
pub(crate) fn convert_i64_to_u64(v: i64, field: &str) -> RepositoryResult<u64> {
    u64::try_from(v).map_err(|e| RepositoryError::Integrity(format!("invalid {field}: {e}")))
}

/// Narrows a `SMALLINT` into a `u8`; fails outside `0..=255`.
pub(crate) fn convert_i16_to_u8(v: i16, field: &str) -> RepositoryResult<u8> {
    u8::try_from(v).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {v}")))
}

/// Lifts any converter above over `Option`, for a nullable column.
pub(crate) fn convert_optional<T, U>(
    value: Option<T>,
    field: &str,
    convert: impl Fn(T, &str) -> RepositoryResult<U>,
) -> RepositoryResult<Option<U>> {
    value.map(|v| convert(v, field)).transpose()
}

/// Converts a `NUMERIC` into a `u128`, for values past `i64` such as `price_q64`.
pub(crate) fn convert_bigdecimal_to_u128(
    bigdecimal: BigDecimal,
    field: &str,
) -> RepositoryResult<u128> {
    bigdecimal
        // ⚠️ Never `to_string()`: sixteen trailing zeros or more are written `…e+16`, refused.
        .to_plain_string()
        .parse::<u128>()
        .map_err(|e| RepositoryError::Integrity(format!("{field} parse error: {e}")))
}

/// Maps a [`sqlx::Error`] to its `RepositoryError`, coarsely: refine a variant
/// only when a caller needs to tell it apart.
pub(crate) fn map_sqlx_error(err: SqlxError) -> RepositoryError {
    match &err {
        SqlxError::RowNotFound => RepositoryError::NotFound(err.to_string()),

        SqlxError::Database(db_err) if db_err.is_unique_violation() => {
            RepositoryError::Conflict(err.to_string())
        }
        SqlxError::Database(db_err) if db_err.is_foreign_key_violation() => {
            RepositoryError::Conflict(err.to_string())
        }

        SqlxError::PoolTimedOut => RepositoryError::Timeout(err.to_string()),

        // `statement_timeout` expired: a time budget, like the pool's, not a backend fault.
        SqlxError::Database(db_err) if db_err.code().as_deref() == Some(QUERY_CANCELED) => {
            RepositoryError::Timeout(err.to_string())
        }

        _ => RepositoryError::Backend(err.to_string()),
    }
}

/// SQLSTATE `57014` (`query_canceled`), what `statement_timeout` raises.
const QUERY_CANCELED: &str = "57014";

/// Converts a `Decimal` to the `BigDecimal` a `NUMERIC` binds to, through its exact string.
pub(crate) fn convert_decimal_to_bigdecimal(
    value: Decimal,
    field: &str,
) -> RepositoryResult<BigDecimal> {
    BigDecimal::from_str(&value.to_string())
        .map_err(|e| RepositoryError::Integrity(format!("invalid {field} decimal: {e}")))
}

/// Converts a `u128` for a `NUMERIC(39, 0)` column, lossless.
pub(crate) fn convert_u128_to_bigdecimal(v: u128, _field: &str) -> BigDecimal {
    BigDecimal::from_str(&v.to_string()).expect("u128 string is always valid BigDecimal")
}

/// Converts a `NUMERIC` into a `Decimal`; one too large fails instead of truncating.
/// ⚠️ `to_plain_string()`, never `to_string()`: `Decimal::from_str` drops the exponent of a
/// long scientific-notation mantissa and returns `Ok`, off by a power of ten.
pub(crate) fn convert_bigdecimal_to_decimal(
    value: BigDecimal,
    field: &str,
) -> RepositoryResult<Decimal> {
    Decimal::from_str(&value.to_plain_string()).map_err(|e| {
        RepositoryError::Integrity(format!(
            "failed to convert {field} from BigDecimal to Decimal: {e}"
        ))
    })
}

#[cfg(test)]
#[path = "tests/parser_tests.rs"]
mod tests;
