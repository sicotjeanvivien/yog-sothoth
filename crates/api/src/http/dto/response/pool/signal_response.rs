use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::SignalRecord;

use crate::http::dto::response::signal::SeverityResponse;

/// One entry of a pool's recent-signals indicator: severity, detector and time
/// only; the full signal lives on the feed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PoolSignalResponse {
    severity: SeverityResponse,
    #[schema(min_length = 1)]
    pub(super) detector: String,
    pub(super) triggered_at: DateTime<Utc>,
}

impl From<SignalRecord> for PoolSignalResponse {
    fn from(record: SignalRecord) -> Self {
        Self {
            severity: record.signal.severity.into(),
            detector: record.signal.detector,
            triggered_at: record.signal.triggered_at,
        }
    }
}
