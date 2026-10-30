//! Response DTO for the signal feed.

mod meaning_response;
mod severity_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

use super::protocol::ProtocolResponse;
use super::token::EmbeddedTokenResponse;
use crate::application::EnrichedSignal;
use crate::http::dto::schema::{Count, SignedDecimal, UnsignedDecimal};

use meaning_response::{PairTokenResponse, SignalValueUnitResponse};
pub(super) use severity_response::SeverityResponse;

/// `GET /api/signals` item (and `GET /api/signals/stream` event — both
/// endpoints emit exactly this object).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SignalResponse {
    /// Storage identity — stable across pages, usable as a client-side
    /// list key. Emitted as a JSON number: signals are sparse, the
    /// sequence will never approach 2^53.
    #[schema(value_type = Count)]
    id: i64,
    #[schema(min_length = 1)]
    detector: String,
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    /// The pool's token pair: every field nullable until yog-context resolves
    /// the pool.
    token_a: EmbeddedTokenResponse,
    token_b: EmbeddedTokenResponse,
    severity: SeverityResponse,
    #[schema(value_type = SignedDecimal)]
    value: String,
    #[schema(value_type = Option<UnsignedDecimal>)]
    threshold: Option<String>,
    value_unit: SignalValueUnitResponse,
    /// The token a flow imbalance's swaps mostly take from the pool, if any.
    flows_toward: Option<PairTokenResponse>,
    message: Option<String>,
    triggered_at: DateTime<Utc>,
}

impl From<EnrichedSignal> for SignalResponse {
    fn from(enriched: EnrichedSignal) -> Self {
        let signal = enriched.record.signal;
        let value_unit = domain::SignalValueUnit::of(&signal.detector).into();
        let flows_toward = signal.flows_toward().map(Into::into);
        Self {
            id: enriched.record.id,
            detector: signal.detector,
            protocol: signal.protocol.into(),
            pool_address: signal.pool_address.to_string(),
            token_a: enriched.token_a.into(),
            token_b: enriched.token_b.into(),
            severity: signal.severity.into(),
            value: signal.value.to_string(),
            threshold: signal.threshold.map(|t| t.to_string()),
            value_unit,
            flows_toward,
            message: signal.message,
            triggered_at: signal.triggered_at,
        }
    }
}

#[cfg(test)]
#[path = "tests/signal_tests.rs"]
mod tests;
