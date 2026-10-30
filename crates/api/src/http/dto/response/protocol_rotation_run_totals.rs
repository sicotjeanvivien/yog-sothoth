//! The row of a protocol's `rotation-runs/totals`.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{Protocol, RotationRunTotals, RunsFirstHour};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolRotationRunTotalsResponse {
    protocol: ProtocolResponse,
    #[schema(value_type = Count)]
    runs: i64,
    /// The distinct pools the runs are of.
    #[schema(value_type = Count)]
    pools: i64,
    /// The runs not extinct yet.
    #[schema(value_type = Count)]
    in_progress: i64,
    /// From start to end; `null` without a run.
    #[schema(value_type = Option<Count>)]
    median_duration_seconds: Option<i64>,
    #[schema(value_type = Option<Count>)]
    p90_duration_seconds: Option<i64>,
    #[schema(value_type = UnsignedDecimal)]
    volume_usd: Decimal,
    /// `null` while no run's first hour ended before the window did: always over `1h`.
    first_hour: Option<RotationRunsFirstHourResponse>,
}

/// The runs whose first hour ended before the window did, their volume, and its part in it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RotationRunsFirstHourResponse {
    #[schema(value_type = Count)]
    runs: i64,
    #[schema(value_type = UnsignedDecimal)]
    volume_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    in_first_hour_usd: Decimal,
}

impl From<RunsFirstHour> for RotationRunsFirstHourResponse {
    fn from(first_hour: RunsFirstHour) -> Self {
        Self {
            runs: first_hour.runs,
            volume_usd: first_hour.volume_usd,
            in_first_hour_usd: first_hour.in_first_hour_usd,
        }
    }
}

impl ProtocolRotationRunTotalsResponse {
    pub(crate) fn new(protocol: Protocol, totals: RotationRunTotals) -> Self {
        Self {
            protocol: protocol.into(),
            runs: totals.runs,
            pools: totals.pools,
            in_progress: totals.in_progress,
            median_duration_seconds: totals.median_duration_seconds,
            p90_duration_seconds: totals.p90_duration_seconds,
            volume_usd: totals.volume_usd,
            first_hour: totals.first_hour.map(Into::into),
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_rotation_run_totals_tests.rs"]
mod tests;
