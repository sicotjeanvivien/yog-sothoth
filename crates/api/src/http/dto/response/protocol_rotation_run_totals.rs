//! The row of a protocol's `rotation-runs/totals`.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{Protocol, RotationRunTotals};

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
    #[schema(value_type = UnsignedDecimal)]
    first_hour_volume_usd: Decimal,
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
            first_hour_volume_usd: totals.first_hour_volume_usd,
        }
    }
}
