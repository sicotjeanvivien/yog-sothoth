use crate::protocols::PoolIndexer;
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};

use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DAMM v1 protocol handler (x·y=k + dual-yield).
/// Phase 2 — stub only.
pub struct MeteoraDammV1;

impl PoolIndexer for MeteoraDammV1 {
    fn program_id(&self) -> Pubkey {
        unimplemented!("Meteora DAMM v1 program ID — Phase 2")
    }

    fn is_swap(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        unimplemented!("Meteora DAMM v1 — Phase 2")
    }

    fn is_add_liquidity(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        unimplemented!("Meteora DAMM v1 — Phase 2")
    }

    fn is_remove_liquidity(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        unimplemented!("Meteora DAMM v1 — Phase 2")
    }

    fn parse_swap(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<SwapEvent> {
        unimplemented!("Meteora DAMM v1 — Phase 2")
    }

    fn parse_add_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent> {
        unimplemented!("Meteora DAMM v1 — Phase 2")
    }

    fn parse_remove_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent> {
        unimplemented!("Meteora DAMM v1 — Phase 2")
    }
}
