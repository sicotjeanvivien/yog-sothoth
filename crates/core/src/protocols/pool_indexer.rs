use crate::{CoreResult, LiquidityEvent, SwapEvent};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Common interface for all supported AMM protocols.
///
/// Each protocol implements this trait. The indexer dispatches
/// incoming transactions to the correct implementation based on `program_id()`.
pub trait PoolIndexer {
    /// The on-chain program ID for this protocol.
    fn program_id(&self) -> Pubkey;

    /// Parse a swap instruction from a confirmed transaction.
    /// Returns `None` if the transaction is not a swap for this protocol.
    fn parse_swap(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<Option<SwapEvent>>;

    /// Parse a liquidity add instruction from a confirmed transaction.
    /// Returns `None` if the transaction is not a liquidity add for this protocol.
    fn parse_add_liquidity(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<Option<LiquidityEvent>>;

    /// Parse a liquidity remove instruction from a confirmed transaction.
    /// Returns `None` if the transaction is not a liquidity remove for this protocol.
    fn parse_remove_liquidity(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<Option<LiquidityEvent>>;
}
