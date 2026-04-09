use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Whether liquidity was added or removed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiquidityEventKind {
    Add,
    Remove,
}

impl LiquidityEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LiquidityEventKind::Add => "add",
            LiquidityEventKind::Remove => "remove",
        }
    }
}

impl std::fmt::Display for LiquidityEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for LiquidityEventKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(LiquidityEventKind::Add),
            "remove" => Ok(LiquidityEventKind::Remove),
            _ => Err(()),
        }
    }
}

/// Parsed liquidity add or remove event produced by the AMM parser.
///
/// Captures the full state transition — amounts deposited or withdrawn and
/// pool reserves before and after — so TVL and imbalance metrics can be
/// derived without re-reading the chain.
///
/// Amounts are expressed in each token's native units (no decimal scaling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityEvent {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Whether liquidity was added or removed.
    pub kind: LiquidityEventKind,

    /// Amount of token A deposited or withdrawn, in native units.
    pub amount_a: u64,

    /// Amount of token B deposited or withdrawn, in native units.
    pub amount_b: u64,

    /// Reserve of token A immediately before the event.
    pub reserve_a_before: u64,

    /// Reserve of token B immediately before the event.
    pub reserve_b_before: u64,

    /// Reserve of token A immediately after the event.
    pub reserve_a_after: u64,

    /// Reserve of token B immediately after the event.
    pub reserve_b_after: u64,

    /// Transaction signature, base58-encoded.
    pub signature: String,

    /// Block timestamp at which the transaction was confirmed.
    pub timestamp: DateTime<Utc>,
}