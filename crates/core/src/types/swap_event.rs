use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// A parsed swap event from an AMM pool.
///
/// Amounts are expressed in the token's native units (no decimals applied).
/// Price is not stored here — it is derived from the pool reserves in PoolState.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEvent {
    /// Pool address (base58).
    pub pool_address: Pubkey,
    /// Mint address of the token sold by the user.
    pub token_in_mint: String,
    /// Mint address of the token bought by the user.
    pub token_out_mint: String,
    /// Amount of token_in, in native units.
    pub amount_in: u64,
    /// Amount of token_out, in native units.
    pub amount_out: u64,
    pub reserve_a_before: u64,
    pub reserve_b_before: u64,
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,
    /// Transaction signature (base58).
    pub signature: String,
    /// Block timestamp.
    pub timestamp: DateTime<Utc>,
}
