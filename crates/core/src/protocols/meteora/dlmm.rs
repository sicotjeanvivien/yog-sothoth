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
}

impl MeteoraDlmm {
    pub fn new() -> Self {
        Self {
            protocol: Protocol::MeteoraDlmm,
        }
    }
}

impl PoolIndexer for MeteoraDlmm {

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
