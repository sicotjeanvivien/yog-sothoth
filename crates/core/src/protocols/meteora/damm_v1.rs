use crate::domain::Protocol;
use crate::protocols::PoolIndexer;
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};

use solana_pubkey::{pubkey, Pubkey};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DAMM v2 program ID.
pub const METEORA_DAMM_V1_PROGRAM_ID: Pubkey =
    pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB");

/// Meteora DAMM v1 protocol handler (x·y=k + dual-yield).
/// Phase 2 — stub only.
pub struct MeteoraDammV1 {
    protocol: Protocol,
}

impl MeteoraDammV1 {
    pub fn new() -> Self {
        Self {
            protocol: Protocol::MeteoraDammV1,
        }
    }
}

impl PoolIndexer for MeteoraDammV1 {

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
