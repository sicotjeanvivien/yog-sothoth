use std::str::FromStr;

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, convert_string_to_pubkey, parse_protocol,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{Severity, Signal, SignalRecord},
};

/// One `signals` row, with the storage `id` the write-side [`Signal`] does not carry.
///
/// [`Signal`]: yog_core::domain::Signal
#[derive(sqlx::FromRow)]
pub(super) struct SignalRow {
    pub(super) id: i64,
    pub(super) detector: String,
    pub(super) protocol: String,
    pub(super) pool_address: String,
    pub(super) severity: String,
    pub(super) value: BigDecimal,
    pub(super) threshold: Option<BigDecimal>,
    pub(super) message: Option<String>,
    pub(super) triggered_at: DateTime<Utc>,
}

impl TryFrom<SignalRow> for SignalRecord {
    type Error = RepositoryError;

    fn try_from(row: SignalRow) -> Result<Self, Self::Error> {
        Ok(SignalRecord {
            id: row.id,
            signal: Signal {
                detector: row.detector,
                protocol: parse_protocol(&row.protocol)?,
                pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                severity: Severity::from_str(&row.severity).map_err(|_| {
                    RepositoryError::Integrity(format!("invalid severity: {}", row.severity))
                })?,
                value: convert_bigdecimal_to_decimal(row.value, "value")?,
                threshold: convert_optional(
                    row.threshold,
                    "threshold",
                    convert_bigdecimal_to_decimal,
                )?,
                message: row.message,
                triggered_at: row.triggered_at,
            },
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
