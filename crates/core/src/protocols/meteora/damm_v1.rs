use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

use crate::domain::Protocol;
use crate::protocols::extraction::ExtractionOutcome;
use crate::protocols::PoolIndexer;
use crate::CoreResult;

/// Meteora DAMM v1 protocol handler (x·y=k + dual-yield).
///
/// Phase 2 — `extract_events` returns an empty outcome.
/// To be replaced with real extraction once DAMM v1 wire events are mirrored.
pub struct MeteoraDammV1 {
    protocol: Protocol,
    program_id_str: String,
}

impl MeteoraDammV1 {
    pub fn new() -> Self {
        let protocol = Protocol::MeteoraDammV1;
        let program_id_str = protocol.program_id().to_string();
        Self {
            protocol,
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
