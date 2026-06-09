use chrono::{DateTime, Utc};
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

/// Raw liquidity-change event extracted from an on-chain Anchor event.
///
/// Covers both add-liquidity and remove-liquidity operations, discriminated
/// by [`liquidity_event_kind`].
///
/// # Conventions
///
/// `amount_a` / `amount_b` and `reserve_a_after` / `reserve_b_after` follow
/// the canonical pool ordering — see [`crate::domain::SwapEvent`] for details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2LiquidityEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub liquidity_event_kind: MeteoraDammV2LiquidityEventKind,
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity_delta: u128,
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,
    pub position: Pubkey,
    pub owner: Pubkey,
}
