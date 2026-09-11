//! Dispatch entry point for protocol-specific event extraction.
//!
//! Given a [`Protocol`], routes the transaction to the right per-protocol
//! handler and returns a unified [`ExtractionOutcome`]. Holds one
//! pre-instantiated handler per supported protocol, so dispatch is a
//! cheap enum match — no dyn dispatch, no allocation per call.

use crate::CoreResult;
use crate::application::extraction::EventExtractor;
use crate::application::extraction::OnChainTransaction;
use crate::application::extraction::{
    ExtractionOutcome,
    meteora::{MeteoraDammV2, MeteoraDlmm},
};
use crate::domain::Protocol;

/// Routes extraction calls to the appropriate per-protocol handler.
pub struct ExtractionDispatcher {
    damm_v2: MeteoraDammV2,
    dlmm: MeteoraDlmm,
}

impl ExtractionDispatcher {
    pub fn new() -> Self {
        Self {
            damm_v2: MeteoraDammV2::new(),
            dlmm: MeteoraDlmm::new(),
        }
    }

    /// Extract every domain event the transaction emitted for the given
    /// protocol. Delegates to the protocol-specific [`PoolIndexer`] impl.
    pub fn extract(
        &self,
        protocol: Protocol,
        tx: &OnChainTransaction,
    ) -> CoreResult<ExtractionOutcome> {
        match protocol {
            Protocol::MeteoraDammV2 => self.damm_v2.extract_events(tx),
            Protocol::MeteoraDlmm => self.dlmm.extract_events(tx),
        }
    }

    /// The protocols this build can actually extract — what the ingestion
    /// should subscribe to.
    ///
    /// ⚠️ **Not [`Protocol::all`], and the difference costs money.** `all` is
    /// what the domain *names*; this is what extraction *handles*. A protocol
    /// whose extractor is a stub returns an empty outcome, so subscribing to
    /// its program id buys a firehose to decode and discard — see
    /// [`EventExtractor::is_implemented`], which each extractor answers for
    /// itself so that the answer lives beside the stub, and is flipped with it,
    /// rather than in a second list somebody has to remember.
    ///
    /// Written as a `match` over `all()` rather than a hand-kept list: a new
    /// protocol can neither be omitted from the iteration nor escape the
    /// compiler's exhaustiveness check. It is deliberately *not* factored with
    /// `extract` behind a `&dyn EventExtractor` — this module promises "a cheap
    /// enum match, no dyn dispatch", and a vtable call per transaction is not
    /// worth paying for a question asked once at start-up.
    pub fn implemented_protocols(&self) -> Vec<Protocol> {
        Protocol::all()
            .iter()
            .copied()
            .filter(|protocol| match protocol {
                Protocol::MeteoraDammV2 => self.damm_v2.is_implemented(),
                Protocol::MeteoraDlmm => self.dlmm.is_implemented(),
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "extraction_dispatcher_tests.rs"]
mod tests;

impl Default for ExtractionDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
