//! Response DTO for `GET /api/network/status`.
//!
//! Kept separate from the domain types, like every other `*Response`
//! in this module — the domain model never leaks into the JSON wire
//! shape.

mod freshness_status_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{FreshnessStatus, NetworkStatus};

use crate::http::dto::schema::U128String;

use freshness_status_response::FreshnessStatusResponse;

/// Where the chain is (slot, probe latency) and whether ingestion keeps up. The
/// two fail apart: the probe's endpoint is not the one ingestion uses.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkStatusResponse {
    /// Latest Solana slot the probe saw.
    #[schema(value_type = U128String)]
    slot: String,

    /// Round-trip latency of the probe's last `getSlot` call, in ms — the
    /// distance to *that* endpoint, not to the one ingestion uses.
    rpc_latency_ms: u32,

    /// When the probe recorded the slot above.
    observed_at: DateTime<Utc>,

    /// Ingestion freshness verdict: "live" | "delayed" | "stale".
    freshness: FreshnessStatusResponse,

    /// Timestamp of the most recent indexed event, if any. `null`
    /// when the database holds no events yet.
    last_event_at: Option<DateTime<Utc>>,
}

impl NetworkStatusResponse {
    /// Assemble the response from its two sources: the persisted
    /// network status and the derived freshness.
    pub(crate) fn new(
        status: NetworkStatus,
        freshness: FreshnessStatus,
        last_event_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            slot: status.slot.to_string(),
            rpc_latency_ms: status.rpc_latency_ms,
            observed_at: status.observed_at,
            freshness: freshness.into(),
            last_event_at,
        }
    }
}

#[cfg(test)]
#[path = "tests/network_status_tests.rs"]
mod tests;
