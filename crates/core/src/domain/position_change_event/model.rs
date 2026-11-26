use solana_pubkey::Pubkey;

use crate::domain::{MeteoraDammV2PositionChange, Protocol};

/// A position opened, closed, locked or split, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum PositionChangeEvent {
    MeteoraDammV2(MeteoraDammV2PositionChange),
}

impl PositionChangeEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }

    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::MeteoraDammV2(e) => e.pool_address(),
        }
    }
}
