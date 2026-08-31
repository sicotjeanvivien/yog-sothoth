use solana_pubkey::Pubkey;

use crate::CoreResult;
use crate::application::extraction::{EventExtractor, ExtractionOutcome, TransactionView};
use crate::domain::Protocol;

/// Meteora DLMM protocol handler (bin-based liquidity, volatility fees).
///
/// Phase 2 — `extract_events` returns an empty outcome.
/// To be replaced with real extraction once DLMM wire events are mirrored.
pub struct MeteoraDlmm {
    _protocol: Protocol,
    program_id: Pubkey,
}

impl MeteoraDlmm {
    pub fn new() -> Self {
        let _protocol = Protocol::MeteoraDlmm;
        Self {
            _protocol,
            program_id: _protocol.program_id(),
        }
    }
}

impl Default for MeteoraDlmm {
    fn default() -> Self {
        Self::new()
    }
}

impl EventExtractor for MeteoraDlmm {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn extract_events(&self, _tx: &TransactionView) -> CoreResult<ExtractionOutcome> {
        // Phase 2 stub — no events extracted yet.
        Ok(ExtractionOutcome::default())
    }
}
