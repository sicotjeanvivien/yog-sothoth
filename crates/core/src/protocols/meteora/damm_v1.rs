use crate::protocols::PoolIndexer;
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};

use solana_pubkey::{pubkey, Pubkey};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DAMM v2 program ID.
pub const METEORA_DAMM_V1_PROGRAM_ID: Pubkey = pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB");

/// Meteora DAMM v1 protocol handler (x·y=k + dual-yield).
/// Phase 2 — stub only.
pub struct MeteoraDammV1 {
    pub pool_address: Pubkey,
    program_id_str: String,
}

impl MeteoraDammV1 {
    pub fn new(pool_address: Pubkey) -> Self {
        Self {
            pool_address,
            program_id_str: METEORA_DAMM_V1_PROGRAM_ID.to_string(),
        }
    }
}

impl PoolIndexer for MeteoraDammV1 {
    fn program_id(&self) -> Pubkey {
        METEORA_DAMM_V1_PROGRAM_ID
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
