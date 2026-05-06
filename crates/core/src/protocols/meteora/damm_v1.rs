use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

use crate::CoreResult;
use crate::domain::Protocol;
use crate::protocols::PoolIndexer;
use crate::protocols::extraction::ExtractionOutcome;

/// Meteora DAMM v1 protocol handler (x·y=k + dual-yield).
///
/// Phase 2 — `extract_events` returns an empty outcome.
/// To be replaced with real extraction once DAMM v1 wire events are mirrored.
pub struct MeteoraDammV1 {
    _protocol: Protocol,
    program_id_str: String,
}

impl MeteoraDammV1 {
    pub fn new() -> Self {
        let _protocol = Protocol::MeteoraDammV1;
        let program_id_str = _protocol.program_id().to_string();
        Self {
            _protocol,
            program_id_str,
        }
    }
}

impl Default for MeteoraDammV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolIndexer for MeteoraDammV1 {
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
