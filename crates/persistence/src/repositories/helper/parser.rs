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

pub(crate) fn convert_bigdecimal_to_decimal(
    value: BigDecimal,
    field: &str,
) -> RepositoryResult<Decimal> {
    Decimal::from_str(&value.to_string()).map_err(|e| {
        RepositoryError::Integrity(format!(
            "failed to convert {field} from BigDecimal to Decimal: {e}"
        ))
    })
}

#[cfg(test)]
#[path = "tests/parser_tests.rs"]
mod tests;
