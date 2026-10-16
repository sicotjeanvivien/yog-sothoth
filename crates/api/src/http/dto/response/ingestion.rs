//! Response DTO for `GET /api/ingestion`.

mod freshness_status_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::IngestionAggregate;

use freshness_status_response::FreshnessStatusResponse;

/// Whether our ingestion keeps up, judged on the last event it wrote.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IngestionResponse {
    /// Ingestion freshness verdict: "live" | "delayed" | "stale".
    freshness: FreshnessStatusResponse,

    /// Timestamp of the most recent indexed event, `null` when the database
    /// holds no events yet.
    last_event_at: Option<DateTime<Utc>>,
}

impl From<IngestionAggregate> for IngestionResponse {
    fn from(agg: IngestionAggregate) -> Self {
        Self {
            freshness: agg.freshness.into(),
            last_event_at: agg.last_event_at,
        }
    }
}

#[cfg(test)]
#[path = "tests/ingestion_tests.rs"]
mod tests;
