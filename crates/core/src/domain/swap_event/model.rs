use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

/// Raw swap event parsed from a DAMM v2 transaction.
///
/// Contains only on-chain data — no derived metrics.
/// Metrics (price, price impact, imbalance, fees collected) are computed
/// by the indexer from this struct and written separately to `pool_metrics`.
///
/// Amounts are in native units (no decimal scaling).
/// Reserve fields always refer to token A / token B as defined in
/// `watched_pools.token_a_mint` / `token_b_mint`, regardless of swap direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEvent {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Mint address of the token sold by the trader (token flowing in).
    pub token_in_mint: Pubkey,

    /// Mint address of the token received by the trader (token flowing out).
    pub token_out_mint: Pubkey,

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

    /// Protocol fee rate in basis points (1 bps = 0.01 %).
    /// `None` when the protocol does not expose fee data in the transaction.
    pub fee_bps: Option<u32>,

    /// Absolute fee amount collected on this swap, in native units of `token_in`.
    /// `None` if not directly exposed by the protocol — the indexer will
    /// derive it as `amount_in * fee_bps / 10_000` before writing to `pool_metrics`.
    pub fee_amount: Option<u64>,

    /// Transaction signature, base58-encoded.
    pub signature: String,

    /// Block timestamp at which the transaction was confirmed.
    pub timestamp: DateTime<Utc>,
}
