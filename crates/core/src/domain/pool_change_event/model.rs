use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::{MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2UpdatePoolFeesEvent, Protocol};

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
}

/// A DAMM v2 pool change: its status set, or its fees updated by an operator.
#[derive(Debug, Clone)]
pub enum MeteoraDammV2PoolChange {
    Status(MeteoraDammV2SetPoolStatusEvent),
    Fees(MeteoraDammV2UpdatePoolFeesEvent),
}

impl MeteoraDammV2PoolChange {
    /// The key its list is sorted on: its instant, signature and index in the transaction.
    pub fn key(&self) -> (DateTime<Utc>, Signature, u16) {
        match self {
            Self::Status(e) => (e.timestamp, e.signature, e.event_index),
            Self::Fees(e) => (e.timestamp, e.signature, e.event_index),
        }
    }

    pub fn slot(&self) -> u64 {
        match self {
            Self::Status(e) => e.slot,
            Self::Fees(e) => e.slot,
        }
    }

    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::Status(e) => e.pool_address,
            Self::Fees(e) => e.pool_address,
        }
    }
}
