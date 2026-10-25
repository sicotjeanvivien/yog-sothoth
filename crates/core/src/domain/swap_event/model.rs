use crate::domain::{MeteoraDammV2SwapEvent, Protocol};

/// A swap, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum SwapEvent {
    MeteoraDammV2(MeteoraDammV2SwapEvent),
}

impl SwapEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}
