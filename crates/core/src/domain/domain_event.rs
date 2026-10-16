use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::Protocol;
use crate::domain::meteora::MeteoraDammV2Event;

/// Every domain event extracted from a transaction: the outer variant is the
/// protocol, the inner enum its event kind, each typed against its own table.
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

    /// Protocol that emitted the event.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }

    /// Stable kind label for metrics and logs; the protocol is a separate label.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MeteoraDammV2(e) => e.kind(),
        }
    }
}
