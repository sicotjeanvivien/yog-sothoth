use crate::protocols::PoolIndexer;
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};
use solana_pubkey::{pubkey, Pubkey};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

/// Meteora DLMM protocol handler (bin-based liquidity, volatility fees).
/// Phase 2 — stub only.
pub struct MeteoraDlmm {
    pub pool_address: Pubkey,
    #[expect(dead_code, reason = "Meteora DLMM program ID — Phase 2")]
    program_id_str: String,
}

impl MeteoraDlmm {
    pub fn new(pool_address: Pubkey) -> Self {
        Self {
            pool_address,
            program_id_str: METEORA_DLMM_PROGRAM_ID.to_string(),
        }
    }
}

impl PoolIndexer for MeteoraDlmm {
    fn program_id(&self) -> Pubkey {
        METEORA_DLMM_PROGRAM_ID
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
