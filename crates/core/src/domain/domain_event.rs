use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::Protocol;
use crate::domain::meteora::MeteoraDammV2Event;

/// Sum type of every domain-level event Yog-Sothoth can extract from a
/// transaction, grouped by protocol.
///
/// First level: the protocol that emitted the event. Second level (in the
/// inner enums like [`MeteoraDammV2Event`]): the specific event kind for
/// that protocol. Per-protocol schemas (and persistence tables) diverge
/// enough that flattening would force NULLs or generic JSON; the two-level
/// shape keeps each event kind strongly typed against its own protocol.
///
/// New protocols (DLMM, Raydium CLMM, Orca Whirlpool, …) will arrive as
/// additional variants at this level.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    MeteoraDammV2(MeteoraDammV2Event),
}

impl DomainEvent {
    /// Pool the event refers to.
    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::MeteoraDammV2(e) => e.pool_address(),
        }
    }

    /// Transaction signature this event came from.
    pub fn signature(&self) -> Signature {
        match self {
            Self::MeteoraDammV2(e) => e.signature(),
        }
    }

    /// Block timestamp at which the source transaction was confirmed.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::MeteoraDammV2(e) => e.timestamp(),
        }
    }

    /// Protocol that emitted the event. Determined by the outer variant.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }

    /// Stable kind label suitable for metrics and structured logs
    /// ("swap" | "liquidity" | "claim_position_fee" | "claim_reward").
    /// Independent of protocol — the protocol is a separate label.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MeteoraDammV2(e) => e.kind(),
        }
    }
}
