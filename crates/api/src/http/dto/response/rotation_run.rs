//! A run of `/api/protocols/{protocol}/rotation-runs` and `/api/pools/{pool}/rotation-runs`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::RotationRun;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::UnsignedDecimal;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RotationRunResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    started_at: DateTime<Utc>,
    /// The end of its last marked tranche.
    ended_at: DateTime<Utc>,
    #[schema(value_type = UnsignedDecimal)]
    volume_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    first_hour_volume_usd: Decimal,
    /// No marked tranche followed within the gap: the run will not grow.
    extinct: bool,
}

impl From<RotationRun> for RotationRunResponse {
    fn from(run: RotationRun) -> Self {
        Self {
            protocol: run.protocol.into(),
            pool_address: run.pool_address.to_string(),
            started_at: run.started_at,
            ended_at: run.ended_at,
            volume_usd: run.volume_usd,
            first_hour_volume_usd: run.first_hour_volume_usd,
            extinct: run.extinct,
        }
    }
}

#[cfg(test)]
#[path = "tests/rotation_run_tests.rs"]
mod tests;
