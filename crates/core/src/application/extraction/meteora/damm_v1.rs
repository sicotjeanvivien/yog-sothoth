use solana_pubkey::Pubkey;

use crate::CoreResult;
use crate::application::extraction::{EventExtractor, ExtractionOutcome, TransactionView};
use crate::domain::Protocol;

/// Meteora DAMM v1 protocol handler (x·y=k + dual-yield).
///
/// Phase 2 — `extract_events` returns an empty outcome.
/// To be replaced with real extraction once DAMM v1 wire events are mirrored.
pub struct MeteoraDammV1 {
    _protocol: Protocol,
    program_id: Pubkey,
}

impl MeteoraDammV1 {
    pub fn new() -> Self {
        let _protocol = Protocol::MeteoraDammV1;
        Self {
            _protocol,
            program_id: _protocol.program_id(),
        }
    }
}

impl Default for MeteoraDammV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl EventExtractor for MeteoraDammV1 {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn extract_events(&self, _tx: &TransactionView) -> CoreResult<ExtractionOutcome> {
        // Phase 2 stub — no events extracted yet.
        Ok(ExtractionOutcome::default())
    }
}
