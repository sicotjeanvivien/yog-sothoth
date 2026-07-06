//! Application-level aggregate composing a signal with the token
//! context of the pool it concerns — the same "resource + embedded
//! tokens" composition as [`EnrichedPool`], keyed by the signal's
//! `pool_address`.
//!
//! Domain-facing: `yog-core` types only, no HTTP/wire concern. The
//! HTTP layer maps it to `SignalResponse` (with the shared
//! `EmbeddedTokenResponse` for each side).
//!
//! [`EnrichedPool`]: crate::application::EnrichedPool

use yog_core::domain::SignalRecord;

use crate::application::EnrichedToken;

/// A persisted signal composed with the two enriched token sides of
/// its pool. Produced by `SignalService`, consumed by the HTTP layer
/// for DTO mapping (paginated feed and SSE stream alike).
#[derive(Debug)]
pub(crate) struct EnrichedSignal {
    pub(crate) record: SignalRecord,
    pub(crate) token_a: EnrichedToken,
    pub(crate) token_b: EnrichedToken,
}

impl EnrichedSignal {
    /// The degraded composition: the signal alone, both sides
    /// unresolved. Used by the SSE path when enrichment fails —
    /// delivering the alert beats displaying the pair.
    pub(crate) fn bare(record: SignalRecord) -> Self {
        Self {
            record,
            token_a: EnrichedToken::unresolved(),
            token_b: EnrichedToken::unresolved(),
        }
    }
}
