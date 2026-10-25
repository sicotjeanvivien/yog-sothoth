use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{ProtocolBucket, RotationRun, RotationRunTotals},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_string_to_pubkey, parse_protocol,
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
            },
            key,
        ))
    }
}

pub(super) struct RotationRunTotalsRow {
    pub(super) runs: i64,
    pub(super) volume_usd: BigDecimal,
    pub(super) first_hour_volume_usd: BigDecimal,
}

impl TryFrom<RotationRunTotalsRow> for RotationRunTotals {
    type Error = RepositoryError;

    fn try_from(row: RotationRunTotalsRow) -> Result<Self, Self::Error> {
        Ok(RotationRunTotals {
            runs: row.runs,
            volume_usd: convert_bigdecimal_to_decimal(row.volume_usd, "volume_usd")?,
            first_hour_volume_usd: convert_bigdecimal_to_decimal(
                row.first_hour_volume_usd,
                "first_hour_volume_usd",
            )?,
        })
    }
}

/// The volume in runs over one bucket.
pub(super) struct RotationRunHourRow {
    pub(super) protocol: String,
    pub(super) bucket_from: DateTime<Utc>,
    pub(super) volume_usd: BigDecimal,
}

impl TryFrom<RotationRunHourRow> for ProtocolBucket<Decimal> {
    type Error = RepositoryError;

    fn try_from(row: RotationRunHourRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBucket {
            protocol: parse_protocol(&row.protocol)?,
            from: row.bucket_from,
            value: convert_bigdecimal_to_decimal(row.volume_usd, "volume_usd")?,
        })
    }
}
