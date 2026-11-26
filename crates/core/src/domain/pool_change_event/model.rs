use solana_pubkey::Pubkey;

use crate::domain::{MeteoraDammV2PoolChange, Protocol};

/// A pool's status or fees changed, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum PoolChangeEvent {
    MeteoraDammV2(MeteoraDammV2PoolChange),
}

impl PoolChangeEvent {
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
