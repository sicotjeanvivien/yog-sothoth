use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

use crate::domain::Protocol;
use crate::protocols::extraction::ExtractionOutcome;
use crate::protocols::PoolIndexer;
use crate::CoreResult;

/// Meteora DLMM protocol handler (bin-based liquidity, volatility fees).
///
/// Phase 2 — `extract_events` returns an empty outcome.
/// To be replaced with real extraction once DLMM wire events are mirrored.
pub struct MeteoraDlmm {
    _protocol: Protocol,
    program_id_str: String,
}

impl MeteoraDlmm {
    pub fn new() -> Self {
        let _protocol = Protocol::MeteoraDlmm;
        let program_id_str = _protocol.program_id().to_string();
        Self {
            _protocol,
            program_id_str,
        }
    }
}

impl Default for MeteoraDlmm {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolIndexer for MeteoraDlmm {
    fn program_id(&self) -> &str {
        &self.program_id_str
    }

    fn extract_events(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome> {
        // Phase 2 stub — no events extracted yet.
        Ok(ExtractionOutcome::default())
    }
}
