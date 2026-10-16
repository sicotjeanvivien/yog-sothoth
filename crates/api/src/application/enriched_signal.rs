//! A signal composed with the token context of its pool, in `yog-core` types, as
//! [`EnrichedPool`] is.
//!
//! [`EnrichedPool`]: crate::application::EnrichedPool

use yog_core::domain::SignalRecord;

use crate::application::EnrichedToken;

/// A persisted signal with its pool's two enriched token sides. `Clone`: the poller shares one
/// behind an `Arc`, and each stream clones it.
#[derive(Debug, Clone)]
pub(crate) struct EnrichedSignal {
    pub(crate) record: SignalRecord,
    pub(crate) token_a: EnrichedToken,
    pub(crate) token_b: EnrichedToken,
}

impl EnrichedSignal {
    /// The signal alone, both sides unresolved: what the poller sends when enrichment fails.
    pub(crate) fn bare(record: SignalRecord) -> Self {
        Self {
            record,
            token_a: EnrichedToken::unresolved(),
            token_b: EnrichedToken::unresolved(),
        }
    }
}
