//! Response DTO for the signal feed.

use chrono::{DateTime, Utc};
use serde::Serialize;

use yog_core::domain::SignalRecord;

/// `GET /api/signals` item.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SignalResponse {
    /// Storage identity — stable across pages, usable as a client-side
    /// list key. Emitted as a JSON number: signals are sparse, the
    /// sequence will never approach 2^53.
    pub(crate) id: i64,
    pub(crate) detector: String,
    pub(crate) protocol: String,
    pub(crate) pool_address: String,
    pub(crate) severity: String,
    // Exact decimals are emitted as strings, like every other decimal
    // quantity of the API (a JSON-number consumer would go through
    // lossy f64).
    pub(crate) value: String,
    pub(crate) threshold: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) triggered_at: DateTime<Utc>,
}

impl From<SignalRecord> for SignalResponse {
    fn from(record: SignalRecord) -> Self {
        let signal = record.signal;
        Self {
            id: record.id,
            detector: signal.detector,
            protocol: signal.protocol.to_string(),
            pool_address: signal.pool_address.to_string(),
            severity: signal.severity.to_string(),
            value: signal.value.to_string(),
            threshold: signal.threshold.map(|t| t.to_string()),
            message: signal.message,
            triggered_at: signal.triggered_at,
        }
    }
}
