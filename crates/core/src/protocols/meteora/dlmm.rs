use crate::protocols::PoolIndexer;
use crate::{CoreResult, LiquidityEvent, SwapEvent};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DLMM protocol handler (bin-based liquidity, volatility fees).
/// Phase 2 — stub only.
pub struct MeteoraDlmm;

impl PoolIndexer for MeteoraDlmm {
    fn program_id(&self) -> Pubkey {
        unimplemented!("Meteora DLMM program ID — Phase 2")
    }

    fn parse_swap(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<Option<SwapEvent>> {
        unimplemented!("Meteora DLMM — Phase 2")
    }

    fn parse_add_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<Option<LiquidityEvent>> {
        unimplemented!("Meteora DLMM — Phase 2")
    }

    fn parse_remove_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<Option<LiquidityEvent>> {
        unimplemented!("Meteora DLMM — Phase 2")
    }
}
