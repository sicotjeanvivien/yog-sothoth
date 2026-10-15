use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::SignalRecord;

use crate::http::dto::response::signal::SeverityResponse;

/// Wire shape of one entry of a pool's recent-signals list — the
/// pools-list signal indicator. Deliberately slimmer than the feed's
/// `SignalResponse`: the indicator needs severity, kind and recency,
/// nothing else; the full signal lives on the pool's Alerts tab.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PoolSignalResponse {
    pub(super) severity: SeverityResponse,
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
