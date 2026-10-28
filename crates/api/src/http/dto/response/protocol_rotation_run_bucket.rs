//! A bucket of a protocol's `rotation-runs/hourly` or `/daily`: its volume, split by the runs.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ProtocolBucket, RunSplit};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::UnsignedDecimal;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolRotationRunBucketResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    /// Every swap of the bucket, as `swaps/hourly`; `null` where none is valued.
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_usd: Option<Decimal>,
    /// `null` where the runs do not cover the bucket whole.
    #[schema(value_type = Option<UnsignedDecimal>)]
    in_runs_usd: Option<Decimal>,
    /// `volumeUsd − inRunsUsd`, never under 0; `null` as soon as one of them is.
    #[schema(value_type = Option<UnsignedDecimal>)]
    outside_runs_usd: Option<Decimal>,
}

impl From<ProtocolBucket<RunSplit>> for ProtocolRotationRunBucketResponse {
    fn from(b: ProtocolBucket<RunSplit>) -> Self {
        let outside_runs_usd = b.value.outside_runs_usd();
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            volume_usd: b.value.volume_usd,
            in_runs_usd: b.value.in_runs_usd,
            outside_runs_usd,
        }
    }
}
