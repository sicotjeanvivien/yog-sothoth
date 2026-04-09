use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Parsed swap event produced by the AMM parser.
///
/// Captures the full state transition of a swap — amounts, reserves before
/// and after, and optional protocol fee — so all downstream metrics (price,
/// slippage, price impact) can be derived without re-reading the chain.
///
/// Amounts are expressed in each token's native units (no decimal scaling).
/// Price is not stored directly; derive it from `reserve_a_after / reserve_b_after`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEvent {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Mint address of the token sold by the trader (token flowing in).
    pub token_in_mint: String,

    /// Mint address of the token received by the trader (token flowing out).
    pub token_out_mint: String,

    /// Amount of `token_in` consumed, in native units.
    pub amount_in: u64,

    /// Amount of `token_out` received, in native units.
    pub amount_out: u64,

    /// Reserve of token A immediately before the swap.
    pub reserve_a_before: u64,

    /// Reserve of token B immediately before the swap.
    pub reserve_b_before: u64,

    /// Reserve of token A immediately after the swap.
    pub reserve_a_after: u64,

    /// Reserve of token B immediately after the swap.
    pub reserve_b_after: u64,

    /// Protocol fee in basis points (1 bps = 0.01 %).
    /// `None` when the protocol does not expose fee data in the transaction.
    pub fee_bps: Option<u32>,

    /// Transaction signature, base58-encoded.
    pub signature: String,

    /// Block timestamp at which the transaction was confirmed.
    pub timestamp: DateTime<Utc>,
}