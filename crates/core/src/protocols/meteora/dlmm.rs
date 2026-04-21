use crate::domain::Protocol;
use crate::protocols::PoolIndexer;
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DLMM protocol handler (bin-based liquidity, volatility fees).
/// Phase 2 — stub only.
pub struct MeteoraDlmm {
    protocol: Protocol,
    program_id_str: String,
}

impl MeteoraDlmm {
    pub fn new() -> Self {
        let protocol = Protocol::MeteoraDammV2;
        let program_id_str = protocol.program_id().to_string();
        Self {
            protocol,
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

    fn is_swap(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        unimplemented!("Meteora DLMM program ID — Phase 2")
    }

    fn is_add_liquidity(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        unimplemented!("Meteora DLMM program ID — Phase 2")
    }

    fn is_remove_liquidity(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        unimplemented!("Meteora DLMM program ID — Phase 2")
    }

    fn parse_swap(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<SwapEvent> {
        unimplemented!("Meteora DLMM — Phase 2")
    }

    fn parse_add_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent> {
        unimplemented!("Meteora DLMM — Phase 2")
    }

    fn parse_remove_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent> {
        unimplemented!("Meteora DLMM — Phase 2")
    }
}
