//! Routes a transaction to its protocol's extractor: an enum match, no dyn dispatch.

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

    /// Extract every domain event the transaction emitted for `protocol`.
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

    /// The protocols this build can actually extract: what ingestion subscribes to.
    /// ⚠️ Not [`Protocol::all`]: a stub's program id is a firehose for nothing
    /// ([`EventExtractor::is_implemented`]).
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
