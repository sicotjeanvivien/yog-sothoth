use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{ProtocolBucket, RotationRun, RotationRunTotals, RunSplit},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, convert_string_to_pubkey, parse_protocol,
};

/// A run, and the volume its page is ordered by: rounded as the cursor holds it.
pub(super) struct RotationRunRow {
    pub(super) protocol: String,
    pub(super) pool_address: String,
    pub(super) started_at: DateTime<Utc>,
    pub(super) ended_at: DateTime<Utc>,
    pub(super) volume_usd: BigDecimal,
    pub(super) first_hour_volume_usd: BigDecimal,
    pub(super) extinct: bool,
    pub(super) volume_key: BigDecimal,
    pub(super) token_a_symbol: Option<String>,
    pub(super) token_b_symbol: Option<String>,
}

impl RotationRunRow {
    pub(super) fn into_keyed(self) -> Result<(RotationRun, Decimal), RepositoryError> {
        let key = convert_bigdecimal_to_decimal(self.volume_key, "volume_key")?;
        Ok((
            RotationRun {
                protocol: parse_protocol(&self.protocol)?,
                pool_address: convert_string_to_pubkey(self.pool_address, "pool_address")?,
                started_at: self.started_at,
                ended_at: self.ended_at,
                volume_usd: convert_bigdecimal_to_decimal(self.volume_usd, "volume_usd")?,
                first_hour_volume_usd: convert_bigdecimal_to_decimal(
                    self.first_hour_volume_usd,
                    "first_hour_volume_usd",
                )?,
                extinct: self.extinct,
                token_a_symbol: self.token_a_symbol,
                token_b_symbol: self.token_b_symbol,
            },
            key,
        ))
    }
}

pub(super) struct RotationRunTotalsRow {
    pub(super) runs: i64,
    pub(super) pools: i64,
    pub(super) in_progress: i64,
    pub(super) median_duration_seconds: Option<i64>,
    pub(super) p90_duration_seconds: Option<i64>,
    pub(super) volume_usd: BigDecimal,
    pub(super) first_hour_volume_usd: BigDecimal,
}

impl TryFrom<RotationRunTotalsRow> for RotationRunTotals {
    type Error = RepositoryError;

    fn try_from(row: RotationRunTotalsRow) -> Result<Self, Self::Error> {
        Ok(RotationRunTotals {
            runs: row.runs,
            pools: row.pools,
            in_progress: row.in_progress,
            median_duration_seconds: row.median_duration_seconds,
            p90_duration_seconds: row.p90_duration_seconds,
            volume_usd: convert_bigdecimal_to_decimal(row.volume_usd, "volume_usd")?,
            first_hour_volume_usd: convert_bigdecimal_to_decimal(
                row.first_hour_volume_usd,
                "first_hour_volume_usd",
            )?,
        })
    }
}

/// A bucket's volume and its part in runs.
pub(super) struct RunSplitRow {
    pub(super) protocol: String,
    pub(super) bucket_from: DateTime<Utc>,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) in_runs_usd: Option<BigDecimal>,
}

impl TryFrom<RunSplitRow> for ProtocolBucket<RunSplit> {
    type Error = RepositoryError;

    fn try_from(row: RunSplitRow) -> Result<Self, Self::Error> {
        let decimal = |value, field| convert_optional(value, field, convert_bigdecimal_to_decimal);
        Ok(ProtocolBucket {
            protocol: parse_protocol(&row.protocol)?,
            from: row.bucket_from,
            value: RunSplit {
                volume_usd: decimal(row.volume_usd, "volume_usd")?,
                in_runs_usd: decimal(row.in_runs_usd, "in_runs_usd")?,
            },
        })
    }
}
