//! Response DTO for the signal feed.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::application::EnrichedSignal;
use crate::http::dto::EmbeddedTokenResponse;

/// `GET /api/signals` item (and `GET /api/signals/stream` event — both
/// endpoints emit exactly this object).
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
    /// The pool's token pair, embedded in the same shape as
    /// `PoolResponse` (`EmbeddedTokenResponse`). Every field inside is
    /// nullable — a pool not yet resolved by yog-context embeds the
    /// minimal view, and the client falls back to `pool_address`.
    pub(crate) token_a: EmbeddedTokenResponse,
    pub(crate) token_b: EmbeddedTokenResponse,
    pub(crate) severity: String,
    // Exact decimals are emitted as strings, like every other decimal
    // quantity of the API (a JSON-number consumer would go through
    // lossy f64).
    pub(crate) value: String,
    pub(crate) threshold: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) triggered_at: DateTime<Utc>,
}

impl From<EnrichedSignal> for SignalResponse {
    fn from(enriched: EnrichedSignal) -> Self {
        let signal = enriched.record.signal;
        Self {
            id: enriched.record.id,
            detector: signal.detector,
            protocol: signal.protocol.to_string(),
            pool_address: signal.pool_address.to_string(),
            token_a: enriched.token_a.into(),
            token_b: enriched.token_b.into(),
            severity: signal.severity.to_string(),
            value: signal.value.to_string(),
            threshold: signal.threshold.map(|t| t.to_string()),
            message: signal.message,
            triggered_at: signal.triggered_at,
        }
    }
}
