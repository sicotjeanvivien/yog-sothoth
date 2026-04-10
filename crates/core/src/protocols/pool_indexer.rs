use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};
use solana_pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Common interface for all supported AMM protocols.
///
/// Each protocol implements this trait. The indexer dispatches
/// incoming transactions to the correct implementation based on `program_id()`.
///
/// # Dispatch contract
///
/// The indexer always calls a discriminant (`is_*`) before its corresponding
/// parser (`parse_*`). Implementations may assume this ordering and skip
/// redundant instruction-type checks inside `parse_*`.
pub trait PoolIndexer {
    /// The on-chain program ID for this protocol.
    fn program_id(&self) -> Pubkey;

    /// Returns `true` if the transaction contains a swap instruction for this protocol.
    fn is_swap(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool;

    /// Returns `true` if the transaction contains a liquidity add instruction for this protocol.
    fn is_add_liquidity(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool;

    /// Returns `true` if the transaction contains a liquidity remove instruction for this protocol.
    fn is_remove_liquidity(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool;

    /// Parse a swap instruction from a confirmed transaction.
    /// Returns `None` if the transaction is not a swap for this protocol.
    fn parse_swap(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<SwapEvent>;

    /// Parse a liquidity add instruction from a confirmed transaction.
    /// Returns `None` if the transaction is not a liquidity add for this protocol.
    fn parse_add_liquidity(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent>;

    /// Parse a liquidity remove instruction from a confirmed transaction.
    /// Returns `None` if the transaction is not a liquidity remove for this protocol.
    fn parse_remove_liquidity(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent>;
}
