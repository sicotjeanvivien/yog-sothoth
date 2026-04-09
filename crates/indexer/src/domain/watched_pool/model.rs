use chrono::{DateTime, Utc};
use yog_core::domain::Protocol;

/// A pool configured for indexing.
#[derive(Debug, Clone)]
pub(crate) struct WatchedPool {
    /// Pool address (base58).
    pub(crate) address: String,
    /// AMM protocol.
    pub(crate) protocol: Protocol,
    /// Mint address of token A.
    pub(crate) token_a_mint: String,
    /// Mint address of token B.
    pub(crate) token_b_mint: String,
    /// Decimal places for token A.
    pub(crate) token_a_decimals: u8,
    /// Decimal places for token B.
    pub(crate) token_b_decimals: u8,
    /// When this pool was added to the watchlist.
    pub(crate) added_at: DateTime<Utc>,
}
