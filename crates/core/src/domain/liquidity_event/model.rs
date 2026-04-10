use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

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

/// Raw liquidity add or remove event parsed from a DAMM v2 transaction.
///
/// Contains only on-chain data — no derived metrics.
/// Metrics (TVL, imbalance) are computed by the indexer from this struct
/// and written separately to `pool_metrics`.
///
/// Amounts are in native units (no decimal scaling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityEvent {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Whether liquidity was added or removed.
    pub liquidity_event_kind: LiquidityEventKind,

    /// Amount of token A deposited or withdrawn, in native units.
    pub amount_a: u64,

    /// Amount of token B deposited or withdrawn, in native units.
    pub amount_b: u64,

    /// Transaction signature, base58-encoded.
    pub signature: String,

    /// Block timestamp at which the transaction was confirmed.
    pub timestamp: DateTime<Utc>,
}
