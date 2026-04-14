use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// A pool configured for indexing.
#[derive(Debug, Clone)]
pub struct WatchedPool {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,
    /// AMM protocol.
    pub protocol: Protocol,
    /// Mint address of token A.
    pub token_a_mint: Pubkey,
    /// Mint address of token B.
    pub token_b_mint: Pubkey,
    /// Decimal places for token A.
    pub token_a_decimals: u8,
    /// Decimal places for token B.
    pub token_b_decimals: u8,
    /// When this pool was added to the watchlist.
    pub added_at: DateTime<Utc>,
}
