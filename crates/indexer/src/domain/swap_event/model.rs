use chrono::{DateTime, Utc};

/// A parsed swap event — DB representation.
#[derive(Debug, Clone)]
pub(crate) struct SwapEvent {
    /// Pool address (base58).
    pub(crate) pool_address: String,
    /// Transaction signature (base58).
    pub(crate) signature: String,
    /// Mint address of the token sold by the user.
    pub(crate) token_in: String,
    /// Mint address of the token bought by the user.
    pub(crate) token_out: String,
    /// Amount of token_in in native units.
    pub(crate) amount_in: u64,
    /// Amount of token_out in native units.
    pub(crate) amount_out: u64,
    /// Fee in basis points — protocol specific, None for unsupported protocols.
    pub(crate) fee_bps: Option<u32>,
    /// Block timestamp.
    pub(crate) timestamp: DateTime<Utc>,
}
