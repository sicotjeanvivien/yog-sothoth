use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// Whether liquidity was added or removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiquidityEventKind {
    Add,
    Remove,
}

impl LiquidityEventKind {
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

impl std::fmt::Display for LiquidityEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for LiquidityEventKind {
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
pub struct LiquidityEvent {
    // ── Identification ──────────────────────────────────────────────────────
    pub pool_address: Pubkey,
    pub protocol: Protocol,
    pub signature: String,
    pub timestamp: DateTime<Utc>,

    // ── Pool tokens (canonical order) ───────────────────────────────────────
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,

    // ── Operation ───────────────────────────────────────────────────────────
    pub liquidity_event_kind: LiquidityEventKind,

    /// Amount of token A added or removed (always positive — sign comes from
    /// `liquidity_event_kind`).
    pub amount_a: u64,

    /// Amount of token B added or removed.
    pub amount_b: u64,

    /// Internal liquidity delta (Q-format). Reflects the share-like quantity
    /// changed by this operation, distinct from the underlying token amounts.
    pub liquidity_delta: u128,

    // ── Post-change pool state (canonical order) ────────────────────────────
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,

    // ── LP context ──────────────────────────────────────────────────────────
    /// On-chain address of the LP position affected.
    pub position: Pubkey,

    /// Owner of the position.
    pub owner: Pubkey,
}
