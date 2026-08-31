//! Dispatch entry point for protocol-specific event extraction.
//!
//! Given a [`Protocol`], routes the transaction to the right per-protocol
//! handler and returns a unified [`ExtractionOutcome`]. Holds one
//! pre-instantiated handler per supported protocol, so dispatch is a
//! cheap enum match — no dyn dispatch, no allocation per call.

use crate::CoreResult;
use crate::application::extraction::EventExtractor;
use crate::application::extraction::TransactionView;
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
        tx: &TransactionView,
    ) -> CoreResult<ExtractionOutcome> {
        match protocol {
            Protocol::MeteoraDammV2 => self.damm_v2.extract_events(tx),
            Protocol::MeteoraDlmm => self.dlmm.extract_events(tx),
        }
    }
}

impl Default for ExtractionDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
