use solana_pubkey::Pubkey;

use crate::domain::{MeteoraDammV2InitializePoolEvent, Protocol};

/// A pool's creation, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum PoolCreationEvent {
    MeteoraDammV2(MeteoraDammV2InitializePoolEvent),
}

impl PoolCreationEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }

    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::MeteoraDammV2(e) => e.pool_address,
        }
    }

    /// The pair it creates, `(token A, token B)` in the program's order.
    pub fn mints(&self) -> (Pubkey, Pubkey) {
        match self {
            Self::MeteoraDammV2(e) => (e.token_a_mint, e.token_b_mint),
        }
    }
}
