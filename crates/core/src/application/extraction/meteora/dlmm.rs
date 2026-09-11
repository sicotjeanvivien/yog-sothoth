use solana_pubkey::Pubkey;

use crate::CoreResult;
use crate::application::extraction::{EventExtractor, ExtractionOutcome, OnChainTransaction};
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

    fn extract_events(&self, _tx: &OnChainTransaction) -> CoreResult<ExtractionOutcome> {
        // Phase 2 stub — no events extracted yet.
        Ok(ExtractionOutcome::default())
    }

    /// ⚠️ **False, and it keeps the ingestion off this program id.**
    ///
    /// `extract_events` above returns an empty outcome, and there is no
    /// `DomainEvent::MeteoraDlmm` for it to return anything *into* — so a
    /// subscription to this protocol would decode and discard every
    /// transaction of one of Solana's busiest programs, paying an RPC quota or
    /// a bandwidth bill for zero rows.
    ///
    /// **Flip this to `true` when the extraction lands**, in the same change
    /// that replaces the stub. The method has no default, so it stays — only
    /// its answer changes — and nothing else has to be edited for the ingestion
    /// to pick the protocol up.
    fn is_implemented(&self) -> bool {
        false
    }
}
