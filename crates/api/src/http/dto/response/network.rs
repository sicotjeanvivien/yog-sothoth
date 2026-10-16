//! Response DTO for `GET /api/network`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::NetworkStatus;

use crate::http::dto::schema::U128String;

/// Where the chain is, as the probe last saw it. The probe's endpoint is not
/// the one ingestion uses: this says nothing about our ingestion.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkResponse {
    /// Latest Solana slot the probe saw.
    #[schema(value_type = U128String)]
    slot: String,

    /// Round-trip latency of the probe's last `getSlot` call, in ms — the
    /// distance to *that* endpoint, not to the one ingestion uses.
    rpc_latency_ms: u32,

    /// When the probe recorded the slot above.
    observed_at: DateTime<Utc>,
}

impl From<NetworkStatus> for NetworkResponse {
    fn from(status: NetworkStatus) -> Self {
        Self {
            slot: status.slot.to_string(),
            rpc_latency_ms: status.rpc_latency_ms,
            observed_at: status.observed_at,
        }
    }
}
