//! Response DTO for `GET /api/network/status`.
//!
//! Kept separate from the domain types, like every other `*Response`
//! in this module — the domain model never leaks into the JSON wire
//! shape.

use chrono::{DateTime, Utc};
use serde::Serialize;

use yog_core::domain::{FreshnessStatus, NetworkStatus};

/// The "Solana Live" panel payload.
///
/// Two concerns combined: the chain link (slot + RPC latency, from
/// the `network_status` singleton) and ingestion freshness (derived
/// from the most recent indexed event).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkStatusResponse {
    /// Latest Solana slot observed by the indexer.
    ///
    /// Serialized as a string: slots are u64 and can exceed the safe
    /// integer range of a JSON number consumer.
    slot: String,

    /// Round-trip latency of the indexer's last `getSlot` call, in ms.
    rpc_latency_ms: u32,

    /// When the indexer recorded the slot above.
    observed_at: DateTime<Utc>,

    /// Ingestion freshness verdict: "live" | "delayed" | "stale".
    freshness: String,

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
            freshness: freshness.as_str().to_string(),
            last_event_at,
        }
    }
}
