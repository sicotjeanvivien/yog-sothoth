use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// Whether liquidity was added or removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeteoraDammV2LiquidityEventKind {
    Add,
    Remove,
}

impl MeteoraDammV2LiquidityEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
        }
    }

    /// Decode from the on-chain `u8` field used by Anchor events.
    /// `0 = Add`, `1 = Remove`.
    pub fn from_u8(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::Add),
            1 => Ok(Self::Remove),
            other => Err(other),
        }
    }
}

impl std::fmt::Display for MeteoraDammV2LiquidityEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for MeteoraDammV2LiquidityEventKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(Self::Add),
            "remove" => Ok(Self::Remove),
            _ => Err(()),
        }
    }
}

/// A liquidity add or remove; amounts and reserves (after it) in the pool's own
/// A/B order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2LiquidityEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub liquidity_event_kind: MeteoraDammV2LiquidityEventKind,
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity_delta: u128,
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,
    pub position: Pubkey,
    pub owner: Pubkey,
}

/// A liquidity event and its USD value at event time, derived on read; `None`
/// when a price or a token's decimals are unknown.
#[derive(Debug, Clone)]
pub struct MeteoraDammV2LiquidityEventValued {
    pub event: MeteoraDammV2LiquidityEvent,
    pub value_usd: Option<Decimal>,
}
