use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Whether liquidity was added or removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidityEventType {
    Add,
    Remove,
}

/// A parsed liquidity add or remove event from an AMM pool.
///
/// Amounts are expressed in native units for each token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityEvent {
    /// Pool address (base58).
    pub pool: String,
    pub event_type: LiquidityEventType,
    /// Amount of token A, in native units.
    pub amount_a: u64,
    /// Amount of token B, in native units.
    pub amount_b: u64,
    /// Transaction signature (base58).
    pub signature: String,
    /// Block timestamp.
    pub timestamp: DateTime<Utc>,
}