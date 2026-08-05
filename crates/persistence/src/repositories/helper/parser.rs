use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::Error as SqlxError;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{RepositoryError, RepositoryResult, domain::MeteoraDammV2LiquidityEventKind};

/// Convert a string read from the database into a `Pubkey`.
///
/// Returns `RepositoryError::Integrity` if the value cannot be parsed —
/// this means the row contains a malformed pubkey, which is a data
/// integrity issue (manual edit, schema drift, or upstream write bug).
pub(crate) fn convert_string_to_pubkey(key: String, field: &str) -> RepositoryResult<Pubkey> {
    Pubkey::from_str(&key)
        .map_err(|e| RepositoryError::Integrity(format!("invalid {field} pubkey: {e}")))
}

/// Convert a `u64` (typically a domain value) into the `i64` Postgres
/// column type. Fails on overflow — values close to `u64::MAX` cannot
/// round-trip through Postgres `BIGINT`.
pub(crate) fn convert_u64_to_i64(v: u64, field: &str) -> RepositoryResult<i64> {
    i64::try_from(v).map_err(|e| RepositoryError::Integrity(format!("invalid {field}: {e}")))
}

/// Convert a `BIGINT` read from Postgres into a `u64`. Fails on negative
/// values — they should never appear if the schema is correct.
pub(crate) fn convert_i64_to_u64(v: i64, field: &str) -> RepositoryResult<u64> {
    u64::try_from(v).map_err(|e| RepositoryError::Integrity(format!("invalid {field}: {e}")))
}

/// Narrow a `SMALLINT` read from Postgres into a `u8` (token decimals).
/// Fails on negative or >255 values — they would mean the row was written
/// by a non-conforming source.
pub(crate) fn convert_i16_to_u8(v: i16, field: &str) -> RepositoryResult<u8> {
    u8::try_from(v).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {v}")))
}

/// Narrow an `INTEGER` read from Postgres into a `u16`.
///
/// The width is deliberate: a `u16` does not fit SMALLINT (32 767 < 65 535), so
/// columns holding one are INTEGER and half their range is unreachable by
/// design. This guard catches a value written outside that convention rather
/// than truncating it.
pub(crate) fn convert_i32_to_u16(v: i32, field: &str) -> RepositoryResult<u16> {
    u16::try_from(v).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {v}")))
}

/// Narrow a `BIGINT` read from Postgres into a `u32`. Same reasoning as
/// [`convert_i32_to_u16`]: a `u32` does not fit INTEGER.
pub(crate) fn convert_i64_to_u32(v: i64, field: &str) -> RepositoryResult<u32> {
    u32::try_from(v).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {v}")))
}

/// Lift any of the guards above over `Option`, for a nullable column.
///
/// Every `TryFrom<Row>` in this crate hits the same shape — a nullable column,
/// one scalar converter, one field name — and each was writing its own two-line
/// wrapper (`percent`, `u16_column`, …). The conversion rules stay in the
/// converters; this only carries the absence through.
///
/// ```text
/// bin_step: convert_optional(row.bin_step, "bin_step", convert_i32_to_u16)?,
/// ```
///
/// Fenced as `text`, not `ignore`: the crate's integration run passes
/// `-- --include-ignored`, which un-ignores doctests too and would try to
/// compile this fragment out of context.
pub(crate) fn convert_optional<T, U>(
    value: Option<T>,
    field: &str,
    convert: impl Fn(T, &str) -> RepositoryResult<U>,
) -> RepositoryResult<Option<U>> {
    value.map(|v| convert(v, field)).transpose()
}

/// Convert a Postgres `NUMERIC` (mapped to `BigDecimal`) into a `u128`.
/// Used for fields like `price_q64` that exceed `i64` range.
pub(crate) fn convert_bigdecimal_to_u128(
    bigdecimal: BigDecimal,
    field: &str,
) -> RepositoryResult<u128> {
    bigdecimal
        .to_string()
        .parse::<u128>()
        .map_err(|e| RepositoryError::Integrity(format!("{field} parse error: {e}")))
}

/// Parse a string column into a `MeteoraDammV2LiquidityEventKind` enum value.
pub(crate) fn parse_string_to_liquidity_event_kind(
    liquidity_event_kind: String,
    field: &str,
) -> RepositoryResult<MeteoraDammV2LiquidityEventKind> {
    liquidity_event_kind
        .parse::<MeteoraDammV2LiquidityEventKind>()
        .map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {liquidity_event_kind}")))
}

/// Map a `sqlx::Error` to its semantic `RepositoryError` counterpart.
///
/// The mapping is intentionally coarse — refine variants only when a
/// caller actually needs to distinguish specific cases.
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

        _ => RepositoryError::Backend(err.to_string()),
    }
}

/// Convert a `u128` into a `BigDecimal`, lossless. Used when binding u128
/// values to PostgreSQL `NUMERIC(39, 0)` columns.
pub(crate) fn convert_u128_to_bigdecimal(v: u128, _field: &str) -> BigDecimal {
    // u128::to_string is always parseable into BigDecimal — infallible in practice.
    BigDecimal::from_str(&v.to_string()).expect("u128 string is always valid BigDecimal")
}

pub(crate) fn convert_string_to_signature(key: String, field: &str) -> RepositoryResult<Signature> {
    Signature::from_str(&key)
        .map_err(|e| RepositoryError::Integrity(format!("invalid {field} signature: {e}")))
}

/// Convert a `NUMERIC` read from Postgres into a `Decimal`.
///
/// ⚠️ **`to_plain_string()`, never `to_string()`.** `BigDecimal`'s `Display`
/// switches to scientific notation for small magnitudes, and
/// `Decimal::from_str` mishandles that form once the mantissa reaches its
/// 28-digit scale: it fills the scale with the mantissa, **drops the exponent,
/// and returns `Ok`**. Measured — `6.283855409573290776726186454503737E-12`
/// parses as `6.2838554095732907767261864545`, a factor of 10¹², silently.
///
/// Short mantissas survive (`1E-12` parses correctly), which is why this went
/// unnoticed: valuation from *observed* prices is terminating arithmetic and
/// rarely reaches 28 significant digits. The implied-price division of
/// migration 002 produces 34-38 of them as a matter of course, turning a
/// dormant defect into a reachable one.
///
/// `to_plain_string()` never uses an exponent, so the parse is exact — and a
/// value genuinely too large for `Decimal` now fails loudly instead of being
/// truncated into a plausible wrong number.
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
