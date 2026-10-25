use crate::domain::{MeteoraDammV2LiquidityEvent, Protocol};

/// A liquidity added to or removed from a pool, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum LiquidityChangeEvent {
    MeteoraDammV2(MeteoraDammV2LiquidityEvent),
}

impl LiquidityChangeEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}
